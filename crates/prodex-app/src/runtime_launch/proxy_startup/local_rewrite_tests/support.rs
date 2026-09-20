use crate::AppPaths;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::{IntoUrl, blocking::RequestBuilder};
use std::collections::VecDeque;
use std::fs;
use std::io::{Cursor, Read};
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};

pub(super) struct TestUpstream {
    pub(super) addr: SocketAddr,
    pub(super) body_rx: mpsc::Receiver<Vec<u8>>,
    pub(super) headers_rx: mpsc::Receiver<Vec<(String, String)>>,
    pub(super) path_rx: mpsc::Receiver<String>,
    _thread: thread::JoinHandle<()>,
}

impl TestUpstream {
    pub(super) fn start() -> Self {
        Self::start_n(1)
    }

    pub(super) fn start_n(request_count: usize) -> Self {
        Self::start_n_with_response(
            request_count,
            "application/json",
            r#"{"id":"resp_test","usage":{"input_tokens":7,"output_tokens":11,"total_tokens":18}}"#,
        )
    }

    fn start_n_with_response(
        request_count: usize,
        content_type: &'static str,
        response_body: &'static str,
    ) -> Self {
        Self::start_with_responses(vec![200; request_count], content_type, response_body)
    }

    fn start_with_responses(
        statuses: Vec<u16>,
        content_type: &'static str,
        response_body: &'static str,
    ) -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test upstream should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test upstream should expose TCP addr");
        let (body_tx, body_rx) = mpsc::channel();
        let (headers_tx, headers_rx) = mpsc::channel();
        let (path_tx, path_rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            for status in statuses {
                let mut request = server.recv().expect("test upstream should receive request");
                let path = request.url().to_string();
                let headers = request
                    .headers()
                    .iter()
                    .map(|header| {
                        (
                            header.field.to_string().to_ascii_lowercase(),
                            header.value.as_str().to_string(),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut body = Vec::new();
                request
                    .as_reader()
                    .read_to_end(&mut body)
                    .expect("test upstream should read request body");
                let _ = path_tx.send(path);
                let _ = headers_tx.send(headers);
                let _ = body_tx.send(body);
                let mut response =
                    TinyResponse::from_string(response_body).with_status_code(status);
                response.add_header(TinyHeader::from_bytes("content-type", content_type).unwrap());
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            body_rx,
            headers_rx,
            path_rx,
            _thread: thread,
        }
    }
}

pub(super) fn temp_root(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("prodex-{name}-{nonce}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    root
}

pub(super) fn write_private_test_secret(
    path: impl AsRef<std::path::Path>,
    text: impl Into<String>,
) -> Result<(), secret_store::SecretError> {
    let path = path.as_ref();
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .expect("test secret parent should be private");
    }
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), text)
}

pub(super) fn app_paths_for_root(root: std::path::PathBuf) -> AppPaths {
    AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    }
}

pub(super) fn wait_for_usage_file(path: &std::path::Path) -> serde_json::Value {
    wait_for_json_file(path)
}

const PERSISTENCE_WAIT_ATTEMPTS: usize = 250;

pub(super) fn wait_for_sqlite_usage_total(path: &std::path::Path, key_name: &str, expected: u64) {
    for _ in 0..PERSISTENCE_WAIT_ATTEMPTS {
        if let Ok(conn) = rusqlite::Connection::open(path) {
            let total = conn
                .query_row(
                    "SELECT requests_total FROM prodex_gateway_virtual_key_usage WHERE key_name = ?1",
                    [key_name],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
                .and_then(|value| u64::try_from(value).ok());
            if total == Some(expected) {
                return;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "sqlite usage total for {key_name} did not reach {expected} at {}",
        path.display()
    );
}

pub(super) fn wait_for_ledger_file_key_response_status(
    path: &std::path::Path,
    key_name: &str,
    expected: u16,
) {
    for _ in 0..PERSISTENCE_WAIT_ATTEMPTS {
        if let Ok(bytes) = fs::read(path) {
            for line in String::from_utf8_lossy(&bytes).lines() {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                    && value["key_name"] == key_name
                    && value["response_status"] == expected
                {
                    return;
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "ledger response status for key {key_name} did not reach {expected} at {}",
        path.display()
    );
}

pub(super) fn wait_for_sqlite_ledger_key_response_status(
    path: &std::path::Path,
    key_name: &str,
    expected: u16,
) {
    for _ in 0..PERSISTENCE_WAIT_ATTEMPTS {
        if let Ok(conn) = rusqlite::Connection::open(path) {
            let status = conn
                .query_row(
                    "SELECT response_status FROM prodex_gateway_billing_ledger WHERE key_name = ?1",
                    [key_name],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .ok()
                .flatten()
                .and_then(|value| u16::try_from(value).ok());
            if status == Some(expected) {
                return;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "sqlite ledger response status for key {key_name} did not reach {expected} at {}",
        path.display()
    );
}

pub(super) fn wait_for_json_file(path: &std::path::Path) -> serde_json::Value {
    for _ in 0..PERSISTENCE_WAIT_ATTEMPTS {
        if let Ok(bytes) = fs::read(path)
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            return value;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("usage file was not written at {}", path.display());
}

pub(super) fn wait_for_text_file(path: &std::path::Path) -> String {
    let mut last_error = None;
    for _ in 0..PERSISTENCE_WAIT_ATTEMPTS {
        match fs::read_to_string(path) {
            Ok(contents) => return contents,
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "text file was not readable at {}: {}",
        path.display(),
        last_error.expect("a failed read should record an error")
    );
}

pub(super) struct TestGuardrailWebhook {
    pub(super) addr: SocketAddr,
    _thread: thread::JoinHandle<()>,
}

impl TestGuardrailWebhook {
    pub(super) fn start_deny(reason: &'static str) -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test webhook should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test webhook should expose TCP addr");
        let thread = thread::spawn(move || {
            for _ in 0..16 {
                let Ok(mut request) = server.recv() else {
                    break;
                };
                let mut body = Vec::new();
                let _ = request.as_reader().read_to_end(&mut body);
                let mut response = TinyResponse::from_string(format!(
                    r#"{{"allow":false,"reason":"{reason}","message":"do-not-log-webhook-message"}}"#
                ))
                .with_status_code(200);
                response.add_header(
                    TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                );
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            _thread: thread,
        }
    }
}
