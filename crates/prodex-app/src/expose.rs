use crate::ExposeArgs;
use anyhow::{Context, Result};
use base64::Engine;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use redaction::redaction_redact_secret_like_text;
use std::collections::VecDeque;
use std::env;
use std::io::{self, BufRead, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use terminal_ui::print_panel;
use tiny_http::Server as TinyServer;

mod ui;

use ui::{expose_html_response, expose_js_response, expose_text_response};

const EXPOSE_SCROLLBACK_LIMIT: usize = 1024 * 1024;
const EXPOSE_MAX_INPUT_BYTES: usize = 16 * 1024;
const EXPOSE_AUTH_BACKOFF: Duration = Duration::from_secs(2);

pub(crate) fn handle_expose(args: ExposeArgs) -> Result<()> {
    let token = expose_random_token()?;
    let pty = ExposePty::spawn(&args)?;
    let server =
        Arc::new(TinyServer::http("127.0.0.1:0").map_err(|err| {
            anyhow::anyhow!("failed to bind expose server on 127.0.0.1:0: {err}")
        })?);
    let listen_addr = server
        .server_addr()
        .to_ip()
        .context("expose server did not expose a TCP listen address")?;
    let local_url = format!("http://{listen_addr}/t/{token}?access_token={token}");
    let shared = Arc::new(ExposeShared {
        token,
        pty,
        active_clients: AtomicUsize::new(0),
        max_clients: args.max_clients.max(1),
        failed_auth: AtomicUsize::new(0),
    });

    let server_for_thread = Arc::clone(&server);
    let shared_for_thread = Arc::clone(&shared);
    let _server_thread = thread::spawn(move || {
        while shared_for_thread.pty.running.load(Ordering::SeqCst) {
            match server_for_thread.recv_timeout(Duration::from_millis(250)) {
                Ok(Some(request)) => {
                    let shared = Arc::clone(&shared_for_thread);
                    thread::spawn(move || handle_expose_request(request, &shared));
                }
                Ok(None) => {}
                Err(_) => break,
            }
        }
    });

    let mut tunnel_status = "disabled".to_string();
    let mut tunnel_url = None;
    let mut tunnel = if args.no_tunnel {
        None
    } else {
        match start_cloudflared_tunnel(&format!("http://{listen_addr}")) {
            Ok(tunnel) => {
                if let Some(url) = &tunnel.url {
                    tunnel_status = "ready".to_string();
                    tunnel_url = Some(format!(
                        "{url}/t/{token}?access_token={token}",
                        token = shared.token
                    ));
                } else {
                    tunnel_status = "cloudflared started; waiting for public URL".to_string();
                }
                Some(tunnel)
            }
            Err(err) => {
                tunnel_status = expose_tunnel_unavailable_status(&err);
                None
            }
        }
    };
    print_expose_status(
        &local_url,
        &shared.token,
        shared.max_clients,
        &tunnel_status,
        tunnel_url.as_deref(),
    )?;

    while shared.pty.running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(250));
    }
    if let Some(tunnel) = tunnel.as_mut() {
        let _ = tunnel.child.kill();
    }
    Ok(())
}

fn print_expose_status(
    local_url: &str,
    token: &str,
    max_clients: usize,
    tunnel_status: &str,
    tunnel_url: Option<&str>,
) -> Result<()> {
    let fields = expose_status_fields(local_url, token, max_clients, tunnel_status, tunnel_url);
    print_panel("Expose", &fields);
    Ok(())
}

fn expose_status_fields(
    local_url: &str,
    token: &str,
    max_clients: usize,
    tunnel_status: &str,
    tunnel_url: Option<&str>,
) -> Vec<(String, String)> {
    let mut fields = vec![
        ("Local URL".to_string(), local_url.to_string()),
        ("Access token".to_string(), token.to_string()),
        (
            "Security".to_string(),
            format!(
                "loopback server, token path + query, CSP, origin check, max clients={max_clients}"
            ),
        ),
        ("Tunnel".to_string(), tunnel_status.to_string()),
    ];
    if let Some(url) = tunnel_url {
        fields.push(("Tunnel URL".to_string(), url.to_string()));
    }
    fields
}

fn expose_tunnel_unavailable_status(err: &anyhow::Error) -> String {
    format!(
        "unavailable: {}; install cloudflared or rerun with --no-tunnel",
        redaction_redact_secret_like_text(&format!("{err:#}"))
    )
}

struct ExposeShared {
    token: String,
    pty: ExposePty,
    active_clients: AtomicUsize,
    max_clients: usize,
    failed_auth: AtomicUsize,
}

struct ExposePty {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    scrollback: Arc<Mutex<VecDeque<u8>>>,
    clients: Arc<Mutex<Vec<Sender<Vec<u8>>>>>,
    running: Arc<AtomicBool>,
}

impl ExposePty {
    fn spawn(args: &ExposeArgs) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: args.rows.max(8),
                cols: args.cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to open PTY")?;
        let mut command = expose_command_builder(args.command.as_deref());
        command.env("TERM", "xterm-256color");
        if let Ok(cwd) = env::current_dir() {
            command.cwd(cwd.as_os_str());
        }
        let mut child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn exposed shell")?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("failed to take PTY writer")?;
        let scrollback = Arc::new(Mutex::new(VecDeque::new()));
        let clients = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicBool::new(true));

        {
            let scrollback = Arc::clone(&scrollback);
            let clients = Arc::clone(&clients);
            let running = Arc::clone(&running);
            thread::spawn(move || {
                let mut buf = [0_u8; 8192];
                while running.load(Ordering::SeqCst) {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => expose_broadcast_output(&scrollback, &clients, &buf[..n]),
                        Err(_) => break,
                    }
                }
                running.store(false, Ordering::SeqCst);
            });
        }
        {
            let running = Arc::clone(&running);
            thread::spawn(move || {
                let _ = child.wait();
                running.store(false, Ordering::SeqCst);
            });
        }

        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            scrollback,
            clients,
            running,
        })
    }
}

struct CloudflaredTunnel {
    child: std::process::Child,
    url: Option<String>,
}

fn start_cloudflared_tunnel(local_url: &str) -> Result<CloudflaredTunnel> {
    let mut child = Command::new("cloudflared")
        .args(["tunnel", "--protocol", "http2", "--url", local_url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn cloudflared")?;
    let (tx, rx) = channel();
    if let Some(stdout) = child.stdout.take() {
        expose_scan_cloudflared_output(stdout, tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        expose_scan_cloudflared_output(stderr, tx);
    }
    let url = rx.recv_timeout(Duration::from_secs(12)).ok();
    Ok(CloudflaredTunnel { child, url })
}

fn expose_scan_cloudflared_output<R>(reader: R, tx: Sender<String>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = io::BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Some(url) = expose_find_trycloudflare_url(&line) {
                        let _ = tx.send(url);
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn handle_expose_request(request: tiny_http::Request, shared: &Arc<ExposeShared>) {
    let url = request.url().to_string();
    if url == "/app.js" {
        let _ = request.respond(expose_js_response());
        return;
    }
    if !expose_request_authorized(&url, &shared.token) {
        let failures = shared.failed_auth.fetch_add(1, Ordering::SeqCst);
        if failures > 2 {
            thread::sleep(EXPOSE_AUTH_BACKOFF);
        }
        let _ = request.respond(expose_text_response(404, "not found"));
        return;
    }
    if url.starts_with("/stream/") {
        handle_expose_stream(request, shared);
    } else if url.starts_with("/input/") {
        handle_expose_input(request, shared);
    } else if url.starts_with("/t/") {
        let _ = request.respond(expose_html_response());
    } else {
        let _ = request.respond(expose_text_response(404, "not found"));
    }
}

fn handle_expose_stream(request: tiny_http::Request, shared: &Arc<ExposeShared>) {
    if shared.active_clients.fetch_add(1, Ordering::SeqCst) >= shared.max_clients {
        shared.active_clients.fetch_sub(1, Ordering::SeqCst);
        let _ = request.respond(expose_text_response(429, "too many clients"));
        return;
    }
    let _guard = ExposeClientGuard(Arc::clone(shared));

    let (tx, rx) = channel::<Vec<u8>>();
    if let Ok(mut clients) = shared.pty.clients.lock() {
        clients.push(tx);
    }

    let mut writer = request.into_writer();
    if write!(
        writer,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n"
    )
    .and_then(|_| writer.flush())
    .is_err()
    {
        return;
    }
    if let Ok(scrollback) = shared.pty.scrollback.lock() {
        let bytes: Vec<u8> = scrollback.iter().copied().collect();
        if !bytes.is_empty() && expose_write_sse(&mut writer, &bytes).is_err() {
            return;
        }
    }
    while shared.pty.running.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(bytes) => {
                if expose_write_sse(&mut writer, &bytes).is_err() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if writer
                    .write_all(b": keepalive\n\n")
                    .and_then(|_| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn handle_expose_input(mut request: tiny_http::Request, shared: &Arc<ExposeShared>) {
    if !expose_origin_allowed(&request) {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    }
    if request.method().as_str() != "POST" {
        let _ = request.respond(expose_text_response(405, "method not allowed"));
        return;
    }
    let mut body = Vec::new();
    match request
        .as_reader()
        .take(EXPOSE_MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut body)
    {
        Ok(_) if body.len() <= EXPOSE_MAX_INPUT_BYTES => {
            expose_write_input(&shared.pty.writer, &body);
            let _ = request.respond(expose_text_response(204, ""));
        }
        Ok(_) => {
            let _ = request.respond(expose_text_response(413, "input too large"));
        }
        Err(_) => {
            let _ = request.respond(expose_text_response(400, "invalid input"));
        }
    }
}

fn expose_write_sse(writer: &mut Box<dyn Write + Send + 'static>, bytes: &[u8]) -> io::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    write!(writer, "data: {encoded}\n\n")?;
    writer.flush()
}

struct ExposeClientGuard(Arc<ExposeShared>);

impl Drop for ExposeClientGuard {
    fn drop(&mut self) {
        self.0.active_clients.fetch_sub(1, Ordering::SeqCst);
    }
}

fn expose_command_builder(command: Option<&str>) -> CommandBuilder {
    match command {
        Some(command) if !command.trim().is_empty() => {
            let shell = env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
            let mut builder = CommandBuilder::new(shell);
            builder.arg("-lc");
            builder.arg(command);
            builder
        }
        _ => CommandBuilder::new(env::var("SHELL").unwrap_or_else(|_| "sh".to_string())),
    }
}

fn expose_broadcast_output(
    scrollback: &Arc<Mutex<VecDeque<u8>>>,
    clients: &Arc<Mutex<Vec<Sender<Vec<u8>>>>>,
    bytes: &[u8],
) {
    if let Ok(mut buffer) = scrollback.lock() {
        buffer.extend(bytes);
        while buffer.len() > EXPOSE_SCROLLBACK_LIMIT {
            buffer.pop_front();
        }
    }
    if let Ok(mut clients) = clients.lock() {
        clients.retain(|client| client.send(bytes.to_vec()).is_ok());
    }
}

fn expose_write_input(writer: &Arc<Mutex<Box<dyn Write + Send>>>, bytes: &[u8]) {
    if bytes.len() > EXPOSE_MAX_INPUT_BYTES {
        return;
    }
    if let Ok(mut writer) = writer.lock() {
        let _ = writer.write_all(bytes);
        let _ = writer.flush();
    }
}

fn expose_request_authorized(url: &str, token: &str) -> bool {
    let Some((path, query)) = url.split_once('?') else {
        return false;
    };
    (path == format!("/t/{token}")
        || path == format!("/stream/{token}")
        || path == format!("/input/{token}"))
        && query
            .split('&')
            .any(|part| part == format!("access_token={token}"))
}

fn expose_origin_allowed(request: &tiny_http::Request) -> bool {
    let host = request.headers().iter().find_map(|header| {
        header
            .field
            .equiv("Host")
            .then(|| header.value.as_str().to_string())
    });
    let origin = request.headers().iter().find_map(|header| {
        header
            .field
            .equiv("Origin")
            .then(|| header.value.as_str().to_string())
    });
    match (host, origin) {
        (_, None) => true,
        (Some(host), Some(origin)) => {
            origin == format!("https://{host}") || origin == format!("http://{host}")
        }
        _ => false,
    }
}

fn expose_random_token() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).context("failed to generate expose access token")?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn expose_find_trycloudflare_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|part| part.starts_with("https://") && part.contains(".trycloudflare.com"))
        .map(|part| part.trim_matches(|ch| ch == ',' || ch == ';').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expose_auth_requires_path_and_query_token() {
        assert!(expose_request_authorized("/t/abc?access_token=abc", "abc"));
        assert!(expose_request_authorized(
            "/stream/abc?x=1&access_token=abc",
            "abc"
        ));
        assert!(expose_request_authorized(
            "/input/abc?access_token=abc",
            "abc"
        ));
        assert!(!expose_request_authorized("/t/abc", "abc"));
        assert!(!expose_request_authorized(
            "/t/abc?access_token=wrong",
            "abc"
        ));
    }

    #[test]
    fn cloudflared_url_parser_extracts_quick_tunnel_url() {
        let line = "2026 INF +--------------------------------------------------------------------------------------------+ https://demo.trycloudflare.com";
        assert_eq!(
            expose_find_trycloudflare_url(line).as_deref(),
            Some("https://demo.trycloudflare.com")
        );
    }

    #[test]
    fn expose_status_fields_contain_server_fields() {
        let fields = expose_status_fields(
            "http://127.0.0.1:7777/t/token?access_token=token",
            "token",
            1,
            "ready",
            Some("https://demo.trycloudflare.com/t/token?access_token=token"),
        );

        assert!(fields.contains(&(
            "Local URL".to_string(),
            "http://127.0.0.1:7777/t/token?access_token=token".to_string()
        )));
        assert!(fields.contains(&("Tunnel".to_string(), "ready".to_string())));
        assert!(fields.iter().any(|(label, value)| {
            label == "Tunnel URL" && value.contains("demo.trycloudflare.com")
        }));
    }

    #[test]
    fn expose_tunnel_status_redacts_secret_like_error() {
        let err = anyhow::anyhow!("failed: Authorization: Bearer expose-tunnel-token");

        let status = expose_tunnel_unavailable_status(&err);

        assert!(status.contains("unavailable:"));
        assert!(status.contains("Authorization: Bearer <redacted>"));
        assert!(!status.contains("expose-tunnel-token"));
    }
}
