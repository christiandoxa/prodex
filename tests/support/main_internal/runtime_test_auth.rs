use super::*;

pub(super) fn runtime_proxy_usage_body(email: &str) -> String {
    runtime_proxy_usage_body_with_remaining(email, 95, 95)
}

pub(super) fn runtime_proxy_usage_body_with_remaining(
    email: &str,
    five_hour_remaining: i64,
    weekly_remaining: i64,
) -> String {
    serde_json::json!({
        "email": email,
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": (100 - five_hour_remaining).clamp(0, 100),
                "reset_at": future_epoch(18_000),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": (100 - weekly_remaining).clamp(0, 100),
                "reset_at": future_epoch(604_800),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

pub(super) fn future_epoch(offset_seconds: i64) -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
        + offset_seconds
}

pub(super) fn write_auth_json(path: &Path, account_id: &str) {
    write_auth_json_with_tokens(path, "test-token", account_id, None, None);
}

pub(super) fn write_auth_json_with_tokens(
    path: &Path,
    access_token: &str,
    account_id: &str,
    refresh_token: Option<&str>,
    last_refresh: Option<&str>,
) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create auth parent dir");
    }
    let mut auth_json = serde_json::json!({
        "tokens": {
            "access_token": access_token,
            "account_id": account_id,
        }
    });
    if let Some(refresh_token) = refresh_token {
        auth_json["tokens"]["refresh_token"] = serde_json::Value::String(refresh_token.to_string());
    }
    if let Some(last_refresh) = last_refresh {
        auth_json["last_refresh"] = serde_json::Value::String(last_refresh.to_string());
    }
    fs::write(path, auth_json.to_string()).expect("failed to write auth.json");
}

pub(super) fn fake_jwt_with_exp(exp: i64) -> String {
    fake_jwt_with_exp_and_account_id(exp, "")
}

pub(super) fn fake_jwt_with_exp_and_account_id(exp: i64, account_id: &str) -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "exp": exp,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id,
            },
        })
        .to_string(),
    );
    format!("{header}.{payload}.signature")
}

#[derive(Clone, Copy)]
pub(super) enum TokenAwareServerMode {
    RuntimeStatus,
    Usage,
}

pub(super) struct TokenAwareServer {
    listen_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    auth_headers: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl TokenAwareServer {
    pub(super) fn start(
        mode: TokenAwareServerMode,
        expected_token: &str,
        expected_account_id: &str,
        success_body: String,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind token-aware server");
        let listen_addr = listener
            .local_addr()
            .expect("failed to read token-aware server address");
        listener
            .set_nonblocking(true)
            .expect("failed to set token-aware server nonblocking");

        let shutdown = Arc::new(AtomicBool::new(false));
        let auth_headers = Arc::new(Mutex::new(Vec::new()));
        let shutdown_flag = Arc::clone(&shutdown);
        let auth_headers_flag = Arc::clone(&auth_headers);
        let expected_token = expected_token.to_string();
        let expected_account_id = expected_account_id.to_string();
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let Some(request) = read_http_request(&mut stream) else {
                            continue;
                        };
                        let path = request
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or("/");
                        let authorization = request_header(&request, "Authorization")
                            .unwrap_or_else(|| "<missing>".to_string());
                        let account_id = request_header(&request, "ChatGPT-Account-Id")
                            .unwrap_or_else(|| "<missing>".to_string());
                        auth_headers_flag
                            .lock()
                            .expect("auth_headers poisoned")
                            .push(authorization.clone());

                        let path_matches = match mode {
                            TokenAwareServerMode::RuntimeStatus => {
                                path.ends_with("/backend-api/status")
                            }
                            TokenAwareServerMode::Usage => {
                                path.ends_with("/backend-api/wham/usage")
                                    || path.ends_with("/api/codex/usage")
                            }
                        };
                        let authorized = authorization == format!("Bearer {expected_token}")
                            && account_id == expected_account_id;
                        let (status_line, body) = if path_matches && authorized {
                            ("HTTP/1.1 200 OK", success_body.clone())
                        } else if path_matches {
                            (
                                "HTTP/1.1 401 Unauthorized",
                                serde_json::json!({ "error": "unauthorized" }).to_string(),
                            )
                        } else {
                            (
                                "HTTP/1.1 404 Not Found",
                                serde_json::json!({ "error": "not_found" }).to_string(),
                            )
                        };
                        let response = format!(
                            "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            listen_addr,
            shutdown,
            auth_headers,
            thread: Some(thread),
        }
    }

    pub(super) fn start_runtime_status(expected_token: &str, expected_account_id: &str) -> Self {
        Self::start(
            TokenAwareServerMode::RuntimeStatus,
            expected_token,
            expected_account_id,
            serde_json::json!({
                "status": "ok",
                "account_id": expected_account_id,
            })
            .to_string(),
        )
    }

    pub(super) fn start_usage(expected_token: &str, expected_account_id: &str, email: &str) -> Self {
        Self::start(
            TokenAwareServerMode::Usage,
            expected_token,
            expected_account_id,
            runtime_proxy_usage_body(email),
        )
    }

    pub(super) fn base_url(&self) -> String {
        format!("http://{}/backend-api", self.listen_addr)
    }

    pub(super) fn auth_headers(&self) -> Vec<String> {
        self.auth_headers
            .lock()
            .expect("auth_headers poisoned")
            .clone()
    }
}

impl Drop for TokenAwareServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.listen_addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) struct AuthRefreshServer {
    listen_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    request_bodies: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl AuthRefreshServer {
    pub(super) fn start(access_token: &str, refresh_token: &str) -> Self {
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("failed to bind auth refresh server");
        let listen_addr = listener
            .local_addr()
            .expect("failed to read auth refresh server address");
        listener
            .set_nonblocking(true)
            .expect("failed to set auth refresh server nonblocking");

        let shutdown = Arc::new(AtomicBool::new(false));
        let request_bodies = Arc::new(Mutex::new(Vec::new()));
        let shutdown_flag = Arc::clone(&shutdown);
        let request_bodies_flag = Arc::clone(&request_bodies);
        let access_token = access_token.to_string();
        let refresh_token = refresh_token.to_string();
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let Some(request) = read_http_request(&mut stream) else {
                            continue;
                        };
                        let body = request
                            .split_once("\r\n\r\n")
                            .map(|(_, body)| body.to_string())
                            .unwrap_or_default();
                        request_bodies_flag
                            .lock()
                            .expect("request_bodies poisoned")
                            .push(body);
                        let response_body = serde_json::json!({
                            "access_token": access_token,
                            "refresh_token": refresh_token,
                        })
                        .to_string();
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            listen_addr,
            shutdown,
            request_bodies,
            thread: Some(thread),
        }
    }

    pub(super) fn url(&self) -> String {
        format!("http://{}/oauth/token", self.listen_addr)
    }

    pub(super) fn request_bodies(&self) -> Vec<String> {
        self.request_bodies
            .lock()
            .expect("request_bodies poisoned")
            .clone()
    }
}

impl Drop for AuthRefreshServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.listen_addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn write_api_key_auth_json(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create auth parent dir");
    }
    fs::write(
        path,
        serde_json::json!({
            "auth_mode": "api_key",
            "OPENAI_API_KEY": "test-api-key",
        })
        .to_string(),
    )
    .expect("failed to write api-key auth.json");
}
