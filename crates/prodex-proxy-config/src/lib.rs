use std::time::Duration;

use anyhow::{Context, Result};

pub const COMPACT_REQUEST_TIMEOUT_IDLE_MULTIPLIER: u64 = 4;

pub fn runtime_upstream_no_proxy(explicit_no_proxy: bool) -> bool {
    explicit_no_proxy
}

pub fn runtime_upstream_proxy_mode_label(explicit_no_proxy: bool) -> &'static str {
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        "disabled"
    } else {
        "system"
    }
}

pub fn build_runtime_upstream_async_http_client(
    explicit_no_proxy: bool,
    connect_timeout_ms: u64,
    stream_idle_timeout_ms: u64,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(connect_timeout_ms))
        .read_timeout(Duration::from_millis(stream_idle_timeout_ms));
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .context("failed to build runtime auto-rotate async HTTP client")
}

pub fn runtime_compact_request_timeout_ms(stream_idle_timeout_ms: u64) -> u64 {
    stream_idle_timeout_ms.saturating_mul(COMPACT_REQUEST_TIMEOUT_IDLE_MULTIPLIER)
}

pub fn build_runtime_upstream_async_http_compact_client(
    explicit_no_proxy: bool,
    connect_timeout_ms: u64,
    stream_idle_timeout_ms: u64,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(connect_timeout_ms))
        .timeout(Duration::from_millis(runtime_compact_request_timeout_ms(
            stream_idle_timeout_ms,
        )));
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .context("failed to build runtime auto-rotate compact async HTTP client")
}

pub fn build_upstream_blocking_http_client(
    context_label: &'static str,
    explicit_no_proxy: bool,
    connect_timeout_ms: u64,
    read_timeout_ms: u64,
) -> Result<reqwest::blocking::Client> {
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_millis(connect_timeout_ms))
        .timeout(Duration::from_millis(read_timeout_ms));
    if runtime_upstream_no_proxy(explicit_no_proxy) {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .with_context(|| format!("failed to build {context_label} client"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;
    use tokio::runtime::Builder as TokioRuntimeBuilder;

    #[test]
    fn compact_request_timeout_matches_codex_idle_multiplier() {
        assert_eq!(
            runtime_compact_request_timeout_ms(250),
            250 * COMPACT_REQUEST_TIMEOUT_IDLE_MULTIPLIER
        );
    }

    #[test]
    fn streaming_async_client_keeps_read_idle_timeout() {
        let idle_timeout_ms = 200;
        let url = spawn_delayed_response_server(Duration::from_millis(450));
        let client = build_runtime_upstream_async_http_client(true, 500, idle_timeout_ms)
            .expect("streaming client should build");
        let runtime = test_tokio_runtime();

        let result = runtime.block_on(async { client.post(url).body("{}").send().await });

        assert!(
            result
                .as_ref()
                .err()
                .is_some_and(reqwest::Error::is_timeout),
            "streaming client should time out on read idle; got {result:?}"
        );
    }

    #[test]
    fn compact_async_client_allows_full_response_timeout_past_idle_timeout() {
        let idle_timeout_ms = 200;
        let url = spawn_delayed_response_server(Duration::from_millis(450));
        let client = build_runtime_upstream_async_http_compact_client(true, 500, idle_timeout_ms)
            .expect("compact client should build");
        let runtime = test_tokio_runtime();

        let response = runtime
            .block_on(async { client.post(url).body("{}").send().await })
            .expect("compact client should wait past stream idle timeout");

        assert_eq!(response.status(), reqwest::StatusCode::OK);
    }

    fn test_tokio_runtime() -> tokio::runtime::Runtime {
        TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime should build")
    }

    fn spawn_delayed_response_server(delay: Duration) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let addr = listener
            .local_addr()
            .expect("test server should expose addr");
        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            read_http_request_headers(&mut stream);
            thread::sleep(delay);
            let body = br#"{"ok":true}"#;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(body);
        });
        format!("http://{addr}/backend-api/codex/responses/compact")
    }

    fn read_http_request_headers(stream: &mut TcpStream) {
        let mut request = Vec::new();
        let mut chunk = [0u8; 256];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => request.extend_from_slice(&chunk[..read]),
            }
        }
    }
}
