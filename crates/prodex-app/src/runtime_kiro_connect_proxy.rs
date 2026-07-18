use crate::{AppPaths, RuntimeConfig, RuntimeWebsocketEnvironment};
use anyhow::{Context, Result, bail};
use std::io;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Builder as TokioRuntimeBuilder;
use tokio::sync::{Semaphore, oneshot};
use tokio::task::JoinSet;
use tokio::time::timeout;

const KIRO_CONNECT_MAX_HEADER_BYTES: usize = 8 * 1024;
const KIRO_CONNECT_MAX_TUNNELS: usize = 32;
const KIRO_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct RuntimeKiroConnectProxy {
    listen_addr: SocketAddr,
    proxy_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for RuntimeKiroConnectProxy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeKiroConnectProxy")
            .field("listen_addr", &self.listen_addr)
            .field("proxy_url", &"<redacted>")
            .finish()
    }
}

impl RuntimeKiroConnectProxy {
    pub(crate) fn dry_run() -> Self {
        Self {
            listen_addr: "127.0.0.1:0".parse().expect("static address should parse"),
            proxy_url: "http://prodex:dry-run@127.0.0.1:0".to_string(),
            shutdown: None,
            worker: None,
        }
    }

    pub(crate) fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    pub(crate) fn proxy_url(&self) -> &str {
        &self.proxy_url
    }
}

impl Drop for RuntimeKiroConnectProxy {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(crate) fn start_runtime_kiro_connect_proxy(
    paths: &AppPaths,
    upstream_no_proxy: bool,
) -> Result<RuntimeKiroConnectProxy> {
    let environment = RuntimeConfig::from_env_policy_and_cli(paths)?.websocket_environment;
    spawn_runtime_kiro_connect_proxy(environment, upstream_no_proxy)
}

fn spawn_runtime_kiro_connect_proxy(
    environment: RuntimeWebsocketEnvironment,
    upstream_no_proxy: bool,
) -> Result<RuntimeKiroConnectProxy> {
    if !upstream_no_proxy
        && let Some(proxy) = environment.proxy_url("https")
        && proxy.scheme() != "http"
    {
        bail!(
            "native Kiro tunneling supports an http:// upstream proxy; configure one or use --no-proxy"
        );
    }

    let listener =
        StdTcpListener::bind("127.0.0.1:0").context("failed to bind native Kiro CONNECT proxy")?;
    listener
        .set_nonblocking(true)
        .context("failed to configure native Kiro CONNECT proxy")?;
    let listen_addr = listener
        .local_addr()
        .context("failed to read native Kiro CONNECT proxy address")?;

    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .context("failed to build native Kiro CONNECT proxy runtime")?;
    let listener = {
        let _runtime_guard = runtime.enter();
        TcpListener::from_std(listener)
            .context("failed to prepare native Kiro CONNECT proxy listener")?
    };

    let mut token = [0_u8; 24];
    getrandom::fill(&mut token).context("failed to generate native Kiro proxy credential")?;
    let token = token
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let expected_authorization =
        runtime_proxy_crate::runtime_websocket_proxy_authorization_header("prodex", Some(&token))
            .context("failed to prepare native Kiro proxy credential")?;
    let proxy_url = format!("http://prodex:{token}@{listen_addr}");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let worker = thread::Builder::new()
        .name("prodex-kiro-connect-proxy".to_string())
        .spawn(move || {
            runtime.block_on(run_runtime_kiro_connect_proxy(
                listener,
                expected_authorization,
                environment,
                upstream_no_proxy,
                shutdown_rx,
            ));
        })
        .context("failed to start native Kiro CONNECT proxy")?;

    Ok(RuntimeKiroConnectProxy {
        listen_addr,
        proxy_url,
        shutdown: Some(shutdown_tx),
        worker: Some(worker),
    })
}

async fn run_runtime_kiro_connect_proxy(
    listener: TcpListener,
    expected_authorization: String,
    environment: RuntimeWebsocketEnvironment,
    upstream_no_proxy: bool,
    mut shutdown: oneshot::Receiver<()>,
) {
    let slots = Arc::new(Semaphore::new(KIRO_CONNECT_MAX_TUNNELS));
    let environment = Arc::new(environment);
    let mut tunnels = JoinSet::new();

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            completed = tunnels.join_next(), if !tunnels.is_empty() => {
                let _ = completed;
            }
            accepted = listener.accept() => {
                let Ok((mut client, _peer)) = accepted else {
                    break;
                };
                let Ok(permit) = Arc::clone(&slots).try_acquire_owned() else {
                    let _ = client
                        .write_all(b"HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
                        .await;
                    continue;
                };
                let expected_authorization = expected_authorization.clone();
                let environment = Arc::clone(&environment);
                tunnels.spawn(async move {
                    let _permit = permit;
                    handle_runtime_kiro_connect_client(
                        &mut client,
                        &expected_authorization,
                        &environment,
                        upstream_no_proxy,
                    )
                    .await;
                });
            }
        }
    }

    tunnels.abort_all();
    while tunnels.join_next().await.is_some() {}
}

async fn handle_runtime_kiro_connect_client(
    client: &mut TcpStream,
    expected_authorization: &str,
    environment: &RuntimeWebsocketEnvironment,
    upstream_no_proxy: bool,
) {
    let request = match read_runtime_kiro_connect_request(client).await {
        Ok(request) => request,
        Err(_) => {
            let _ = client
                .write_all(
                    b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                )
                .await;
            return;
        }
    };
    if request.proxy_authorization.as_deref() != Some(expected_authorization) {
        let _ = client
            .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"prodex\"\r\nConnection: close\r\nContent-Length: 0\r\n\r\n")
            .await;
        return;
    }

    let mut upstream = match connect_runtime_kiro_target(
        &request.host,
        request.port,
        &request.authority,
        environment,
        upstream_no_proxy,
    )
    .await
    {
        Ok(upstream) => upstream,
        Err(_) => {
            let _ = client
                .write_all(
                    b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                )
                .await;
            return;
        }
    };

    if client
        .write_all(b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: prodex\r\n\r\n")
        .await
        .is_err()
    {
        return;
    }
    let _ = tokio::io::copy_bidirectional(client, &mut upstream).await;
}

struct RuntimeKiroConnectRequest {
    authority: String,
    host: String,
    port: u16,
    proxy_authorization: Option<String>,
}

async fn read_runtime_kiro_connect_request(
    client: &mut TcpStream,
) -> io::Result<RuntimeKiroConnectRequest> {
    let bytes = read_runtime_kiro_http_header(client).await?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid CONNECT request"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing CONNECT request"))?;
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("CONNECT") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native Kiro proxy only accepts CONNECT",
        ));
    }
    let authority = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing CONNECT authority"))?;
    if !matches!(parts.next(), Some("HTTP/1.0" | "HTTP/1.1")) || parts.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid CONNECT request line",
        ));
    }
    if authority
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'?' | b'#' | b'@'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid CONNECT authority",
        ));
    }
    let parsed = reqwest::Url::parse(&format!("https://{authority}/"))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid CONNECT authority"))?;
    let host = parsed
        .host_str()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing CONNECT host"))?;
    let port = parsed.port_or_known_default().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "missing CONNECT target port")
    })?;
    let proxy_authorization = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("proxy-authorization")
            .then(|| value.trim().to_string())
    });

    Ok(RuntimeKiroConnectRequest {
        authority: runtime_proxy_crate::runtime_websocket_authority(&host, port),
        host,
        port,
        proxy_authorization,
    })
}

async fn read_runtime_kiro_http_header(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut header = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 512];
    loop {
        let read = timeout(KIRO_CONNECT_TIMEOUT, stream.read(&mut chunk))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "CONNECT header timed out"))??;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "CONNECT client closed before headers",
            ));
        }
        header.extend_from_slice(&chunk[..read]);
        if header.len() > KIRO_CONNECT_MAX_HEADER_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "CONNECT headers are too large",
            ));
        }
        if header.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(header);
        }
    }
}

async fn connect_runtime_kiro_target(
    host: &str,
    port: u16,
    authority: &str,
    environment: &RuntimeWebsocketEnvironment,
    upstream_no_proxy: bool,
) -> io::Result<TcpStream> {
    let proxy = if upstream_no_proxy || environment.no_proxy_matches(host, port) {
        None
    } else {
        environment.proxy_url("https")
    };
    let Some(proxy) = proxy else {
        return connect_runtime_kiro_tcp(host, port).await;
    };
    if proxy.scheme() != "http" {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported upstream proxy scheme",
        ));
    }
    let proxy_host = proxy
        .host_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "upstream proxy has no host"))?;
    let proxy_port = proxy
        .port_or_known_default()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "upstream proxy has no port"))?;
    let mut stream = connect_runtime_kiro_tcp(proxy_host, proxy_port).await?;
    let authorization = runtime_proxy_crate::runtime_websocket_proxy_authorization_header(
        proxy.username(),
        proxy.password(),
    );
    let request = runtime_proxy_crate::runtime_websocket_http_connect_request(
        authority,
        authorization.as_deref(),
    );
    timeout(KIRO_CONNECT_TIMEOUT, stream.write_all(request.as_bytes()))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "upstream CONNECT timed out"))??;
    let response = read_runtime_kiro_http_header(&mut stream).await?;
    let status = std::str::from_utf8(&response)
        .ok()
        .and_then(|text| text.lines().next())
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid upstream CONNECT"))?;
    if status != 200 {
        return Err(io::Error::other("upstream proxy rejected CONNECT"));
    }
    Ok(stream)
}

async fn connect_runtime_kiro_tcp(host: &str, port: u16) -> io::Result<TcpStream> {
    timeout(KIRO_CONNECT_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "target connect timed out"))?
}

#[cfg(test)]
mod tests {
    use super::spawn_runtime_kiro_connect_proxy;
    use crate::RuntimeWebsocketEnvironment;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener as StdTcpListener, TcpStream as StdTcpStream};
    use std::thread;

    #[test]
    fn authenticated_connect_tunnel_relays_bytes() {
        let target = StdTcpListener::bind("127.0.0.1:0").unwrap();
        let target_addr = target.local_addr().unwrap();
        let target_worker = thread::spawn(move || {
            let (mut stream, _) = target.accept().unwrap();
            let mut request = [0_u8; 4];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"ping");
            stream.write_all(b"pong").unwrap();
        });
        let proxy =
            spawn_runtime_kiro_connect_proxy(RuntimeWebsocketEnvironment::direct(), true).unwrap();
        let proxy_url = reqwest::Url::parse(proxy.proxy_url()).unwrap();
        let authorization = runtime_proxy_crate::runtime_websocket_proxy_authorization_header(
            proxy_url.username(),
            proxy_url.password(),
        )
        .unwrap();
        let request = runtime_proxy_crate::runtime_websocket_http_connect_request(
            &target_addr.to_string(),
            Some(&authorization),
        );
        let mut client = StdTcpStream::connect(proxy.listen_addr()).unwrap();
        client.write_all(request.as_bytes()).unwrap();
        let mut response = Vec::new();
        let mut byte = [0_u8; 1];
        while !response.ends_with(b"\r\n\r\n") {
            client.read_exact(&mut byte).unwrap();
            response.push(byte[0]);
        }
        assert!(response.starts_with(b"HTTP/1.1 200"));
        client.write_all(b"ping").unwrap();
        let mut reply = [0_u8; 4];
        client.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"pong");
        let _ = client.shutdown(Shutdown::Both);
        drop(proxy);
        target_worker.join().unwrap();
    }

    #[test]
    fn connect_tunnel_requires_its_launch_credential() {
        let proxy =
            spawn_runtime_kiro_connect_proxy(RuntimeWebsocketEnvironment::direct(), true).unwrap();
        let mut client = StdTcpStream::connect(proxy.listen_addr()).unwrap();
        client
            .write_all(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n")
            .unwrap();
        let mut response = [0_u8; 128];
        let read = client.read(&mut response).unwrap();
        assert!(response[..read].starts_with(b"HTTP/1.1 407"));
    }
}
