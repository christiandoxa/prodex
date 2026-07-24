use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use std::ffi::OsString;
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use terminal_ui::print_panel;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use super::DashboardServer;
use crate::{AppPaths, DashboardArgs};

const DASHBOARD_MAX_JSON_BODY_BYTES: usize = 64 * 1024;
const DASHBOARD_REQUEST_QUEUE_CAPACITY: usize = 32;
const DASHBOARD_WORKER_COUNT: usize = 4;

#[derive(Debug)]
struct DashboardJsonBodyTooLarge {
    limit: usize,
}

impl std::fmt::Display for DashboardJsonBodyTooLarge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "dashboard request body exceeds {} bytes",
            self.limit
        )
    }
}

impl std::error::Error for DashboardJsonBodyTooLarge {}

pub(crate) fn serve_dashboard(paths: AppPaths, args: DashboardArgs) -> Result<()> {
    if !is_local_dashboard_host(args.host.trim()) {
        anyhow::bail!(
            "dashboard only accepts loopback --host values; use an SSH tunnel for remote access"
        );
    }
    let bind = format!("{}:{}", args.host.trim(), args.port);
    let (server, fallback_warning) = match Server::http(&bind) {
        Ok(server) => (server, None),
        Err(primary_error) if args.fallback_port && args.port != 0 => {
            let fallback_bind = format!("{}:0", args.host.trim());
            let server = Server::http(&fallback_bind).map_err(|fallback_error| {
                anyhow!(
                    "failed to bind {bind}: {primary_error}; fallback {fallback_bind} failed: {fallback_error}"
                )
            })?;
            (
                server,
                Some(format!(
                    "port {} was unavailable; using an OS-assigned port",
                    args.port
                )),
            )
        }
        Err(err) => return Err(anyhow!("failed to bind {bind}: {err}")),
    };
    let url = dashboard_url(server.server_addr());
    print_dashboard_status(&url, fallback_warning.as_deref())?;
    if args.open
        && let Err(err) = open_browser(&url)
    {
        eprintln!("failed to open the dashboard browser: {err:#}\nOpen {url} manually.");
    }

    let dashboard = Arc::new(DashboardServer {
        paths,
        base_url: args.base_url,
    });
    let (request_tx, request_rx) = mpsc::sync_channel(DASHBOARD_REQUEST_QUEUE_CAPACITY);
    let request_rx = Arc::new(Mutex::new(request_rx));
    let mut workers = Vec::with_capacity(DASHBOARD_WORKER_COUNT);
    for worker_index in 0..DASHBOARD_WORKER_COUNT {
        let dashboard = Arc::clone(&dashboard);
        let request_rx = Arc::clone(&request_rx);
        workers.push(
            thread::Builder::new()
                .name(format!("prodex-dashboard-{worker_index}"))
                .spawn(move || {
                    loop {
                        let request = request_rx
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .recv();
                        let Ok(request) = request else {
                            break;
                        };
                        let result = match dashboard_request_rejection(&request) {
                            Some((status, body)) => {
                                respond_status(request, status, "application/json", body.to_vec())
                            }
                            None => dashboard.handle(request),
                        };
                        if let Err(err) = result {
                            eprintln!("dashboard request failed: {err:#}");
                        }
                    }
                })
                .context("failed to start dashboard worker")?,
        );
    }
    for request in server.incoming_requests() {
        if request_tx.send(request).is_err() {
            break;
        }
    }
    drop(request_tx);
    for worker in workers {
        let _ = worker.join();
    }
    Ok(())
}

fn print_dashboard_status(url: &str, warning: Option<&str>) -> Result<()> {
    print_panel("Dashboard", &dashboard_status_fields(url, warning))?;
    Ok(())
}

pub(super) fn dashboard_status_fields(url: &str, warning: Option<&str>) -> Vec<(String, String)> {
    let mut fields = vec![("URL".to_string(), url.to_string())];
    if let Some(warning) = warning {
        fields.push(("Warning".to_string(), warning.to_string()));
    }
    fields
}

fn dashboard_url(addr: tiny_http::ListenAddr) -> String {
    match addr.to_ip() {
        Some(SocketAddr::V4(addr)) => {
            let host = if addr.ip().is_unspecified() {
                std::net::Ipv4Addr::LOCALHOST
            } else {
                *addr.ip()
            };
            format!("http://{host}:{}", addr.port())
        }
        Some(SocketAddr::V6(addr)) => {
            let host = if addr.ip().is_unspecified() {
                std::net::Ipv6Addr::LOCALHOST
            } else {
                *addr.ip()
            };
            format!("http://[{host}]:{}", addr.port())
        }
        None => "http://127.0.0.1:8765".to_string(),
    }
}

fn is_local_dashboard_host(host: &str) -> bool {
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

pub(super) fn dashboard_request_rejection(
    request: &Request,
) -> Option<(StatusCode, &'static [u8])> {
    let Some(host) = dashboard_request_header(request, "host") else {
        return Some((StatusCode(403), br#"{"error":"forbidden_host"}"#));
    };
    if !dashboard_authority_is_local(host) {
        return Some((StatusCode(403), br#"{"error":"forbidden_host"}"#));
    }

    if matches!(request.method(), Method::Post | Method::Delete) {
        let Some(origin_authority) = dashboard_request_header(request, "origin")
            .and_then(|origin| origin.strip_prefix("http://"))
        else {
            return Some((StatusCode(403), br#"{"error":"forbidden_origin"}"#));
        };
        if !origin_authority.eq_ignore_ascii_case(host) {
            return Some((StatusCode(403), br#"{"error":"forbidden_origin"}"#));
        }
    }

    if request.method() == &Method::Post
        && !dashboard_request_header(request, "content-type").is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Some((StatusCode(415), br#"{"error":"unsupported_media_type"}"#));
    }
    None
}

fn dashboard_request_header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str().trim())
        .filter(|value| !value.is_empty())
}

fn dashboard_authority_is_local(authority: &str) -> bool {
    authority
        .parse::<SocketAddr>()
        .is_ok_and(|addr| addr.ip().is_loopback())
        || authority.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        || authority.rsplit_once(':').is_some_and(|(host, port)| {
            port.parse::<u16>().is_ok() && is_local_dashboard_host(host)
        })
        || is_local_dashboard_host(authority)
}

pub(crate) fn open_browser(url: &str) -> Result<()> {
    let mut command =
        if let Some(browser) = env::var_os("BROWSER").filter(|value| !value.is_empty()) {
            let mut command = Command::new(browser);
            command.arg(url);
            command
        } else {
            let (program, args) = dashboard_browser_command(url);
            let mut command = Command::new(program);
            command.args(args);
            command
        };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start browser")?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn dashboard_browser_command(url: &str) -> (&'static str, Vec<OsString>) {
    ("open", vec![OsString::from(url)])
}

#[cfg(target_os = "windows")]
pub(super) fn dashboard_browser_command(url: &str) -> (&'static str, Vec<OsString>) {
    (
        "rundll32",
        vec![
            OsString::from("url.dll,FileProtocolHandler"),
            OsString::from(url),
        ],
    )
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn dashboard_browser_command(url: &str) -> (&'static str, Vec<OsString>) {
    ("xdg-open", vec![OsString::from(url)])
}

pub(super) fn read_json_body<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T> {
    let body =
        read_dashboard_json_body_limited(request.as_reader(), DASHBOARD_MAX_JSON_BODY_BYTES)?;
    serde_json::from_slice(&body).context("invalid JSON request body")
}

pub(super) fn dashboard_json_body_error_status(err: &anyhow::Error) -> StatusCode {
    if err.downcast_ref::<DashboardJsonBodyTooLarge>().is_some() {
        StatusCode(413)
    } else {
        StatusCode(400)
    }
}

pub(super) fn read_dashboard_json_body_limited(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let mut reader = reader.take((limit as u64).saturating_add(1));
    reader
        .read_to_end(&mut body)
        .context("failed to read dashboard request body")?;
    if body.len() > limit {
        return Err(DashboardJsonBodyTooLarge { limit }.into());
    }
    Ok(body)
}

pub(super) fn respond_json(request: Request, value: Value) -> Result<()> {
    let body = serde_json::to_vec(&value).context("failed to serialize dashboard JSON")?;
    respond_status(request, StatusCode(200), "application/json", body)
}

pub(super) fn respond_json_result(request: Request, result: Result<Value>) -> Result<()> {
    match result {
        Ok(value) => respond_json(request, value),
        Err(err) => respond_error(request, StatusCode(500), err),
    }
}

pub(super) fn respond_error(
    request: Request,
    status: StatusCode,
    err: anyhow::Error,
) -> Result<()> {
    let message = redaction::redaction_redact_secret_like_text(&err.to_string());
    respond_status(
        request,
        status,
        "application/json",
        serde_json::to_vec(&json!({ "error": message }))
            .context("failed to serialize dashboard error")?,
    )
}

pub(super) fn respond_html(request: Request, html: &str) -> Result<()> {
    respond_status(
        request,
        StatusCode(200),
        "text/html; charset=utf-8",
        html.as_bytes().to_vec(),
    )
}

pub(super) fn respond_status(
    request: Request,
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> Result<()> {
    let content_type = Header::from_bytes("content-type", content_type)
        .map_err(|_| anyhow!("failed to build dashboard content-type header"))?;
    let cache_control = Header::from_bytes("cache-control", "no-store")
        .map_err(|_| anyhow!("failed to build dashboard cache-control header"))?;
    let content_type_options = Header::from_bytes("x-content-type-options", "nosniff")
        .map_err(|_| anyhow!("failed to build dashboard content-type-options header"))?;
    let referrer_policy = Header::from_bytes("referrer-policy", "no-referrer")
        .map_err(|_| anyhow!("failed to build dashboard referrer-policy header"))?;
    let content_security_policy = Header::from_bytes(
        "content-security-policy",
        "default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
    )
    .map_err(|_| anyhow!("failed to build dashboard content-security-policy header"))?;
    let response = Response::from_data(body)
        .with_status_code(status)
        .with_header(content_type)
        .with_header(cache_control)
        .with_header(content_type_options)
        .with_header(referrer_policy)
        .with_header(content_security_policy);
    request
        .respond(response)
        .map_err(|err| anyhow!("failed to send dashboard response: {err}"))
}

pub(super) fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            output.push((high << 4) | low);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
