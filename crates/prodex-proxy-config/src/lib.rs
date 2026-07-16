use std::time::Duration;

use anyhow::{Context, Result};

pub const COMPACT_REQUEST_TIMEOUT_IDLE_MULTIPLIER: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamProxyMode {
    System,
    Disabled,
}

impl UpstreamProxyMode {
    pub fn from_explicit_no_proxy(explicit_no_proxy: bool) -> Self {
        if explicit_no_proxy {
            Self::Disabled
        } else {
            Self::System
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncResponseTimeout {
    ReadIdle(Duration),
    Request(Duration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncUpstreamClientConfig {
    pub proxy_mode: UpstreamProxyMode,
    pub connect_timeout: Duration,
    pub response_timeout: AsyncResponseTimeout,
}

pub fn build_runtime_upstream_async_http_client(
    config: AsyncUpstreamClientConfig,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder().connect_timeout(config.connect_timeout);
    builder = match config.response_timeout {
        AsyncResponseTimeout::ReadIdle(timeout) => builder.read_timeout(timeout),
        AsyncResponseTimeout::Request(timeout) => builder.timeout(timeout),
    };
    if config.proxy_mode == UpstreamProxyMode::Disabled {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .context("failed to build runtime upstream async HTTP client")
}

pub fn runtime_compact_request_timeout_ms(stream_idle_timeout_ms: u64) -> u64 {
    stream_idle_timeout_ms.saturating_mul(COMPACT_REQUEST_TIMEOUT_IDLE_MULTIPLIER)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockingUpstreamClientConfig {
    pub context_label: &'static str,
    pub proxy_mode: UpstreamProxyMode,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

pub fn build_upstream_blocking_http_client(
    config: BlockingUpstreamClientConfig,
) -> Result<reqwest::blocking::Client> {
    let mut builder = reqwest::blocking::Client::builder()
        .connect_timeout(config.connect_timeout)
        .timeout(config.request_timeout);
    if config.proxy_mode == UpstreamProxyMode::Disabled {
        builder = builder.no_proxy();
    }
    builder
        .build()
        .with_context(|| format!("failed to build {} client", config.context_label))
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
        let client = build_runtime_upstream_async_http_client(AsyncUpstreamClientConfig {
            proxy_mode: UpstreamProxyMode::Disabled,
            connect_timeout: Duration::from_millis(500),
            response_timeout: AsyncResponseTimeout::ReadIdle(Duration::from_millis(
                idle_timeout_ms,
            )),
        })
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
        let compact_timeout_ms = runtime_compact_request_timeout_ms(idle_timeout_ms);
        let client = build_runtime_upstream_async_http_client(AsyncUpstreamClientConfig {
            proxy_mode: UpstreamProxyMode::Disabled,
            connect_timeout: Duration::from_millis(500),
            response_timeout: AsyncResponseTimeout::Request(Duration::from_millis(
                compact_timeout_ms,
            )),
        })
        .expect("compact client should build");
        let runtime = test_tokio_runtime();

        let response = runtime
            .block_on(async { client.post(url).body("{}").send().await })
            .expect("compact client should wait past stream idle timeout");

        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(compact_timeout_ms, 800);
    }

    #[test]
    fn compact_async_client_uses_explicit_request_timeout() {
        let url = spawn_delayed_response_server(Duration::from_millis(450));
        let client = build_runtime_upstream_async_http_client(AsyncUpstreamClientConfig {
            proxy_mode: UpstreamProxyMode::Disabled,
            connect_timeout: Duration::from_millis(500),
            response_timeout: AsyncResponseTimeout::Request(Duration::from_millis(200)),
        })
        .expect("compact client should build");
        let runtime = test_tokio_runtime();

        let result = runtime.block_on(async { client.post(url).body("{}").send().await });

        assert!(
            result
                .as_ref()
                .err()
                .is_some_and(reqwest::Error::is_timeout),
            "compact client should use explicit request timeout; got {result:?}"
        );
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
