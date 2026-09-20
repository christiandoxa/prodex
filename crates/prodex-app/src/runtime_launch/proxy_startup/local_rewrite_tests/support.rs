use crate::AppPaths;
use std::fs;
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
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
