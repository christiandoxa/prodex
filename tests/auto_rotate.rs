#[path = "support/test_wait.rs"]
mod test_wait;

use base64::Engine as _;
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct TestDir {
    path: PathBuf,
}

static TEST_DIR_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl TestDir {
    fn new() -> Self {
        for _ in 0..32 {
            let unique = format!(
                "prodex-test-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after unix epoch")
                    .as_nanos(),
                TEST_DIR_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            );
            let path = std::env::temp_dir().join(unique);
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => panic!("failed to create temp dir: {err}"),
            }
        }
        panic!("failed to allocate unique temp dir after repeated collisions");
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct UsageServer {
    listen_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    response_delay_ms: Arc<AtomicU64>,
    max_concurrent_requests: Arc<AtomicUsize>,
    thread: Option<JoinHandle<()>>,
}

impl UsageServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind usage server");
        let listen_addr = listener
            .local_addr()
            .expect("failed to resolve usage server address");
        listener
            .set_nonblocking(true)
            .expect("failed to set usage server nonblocking");

        let shutdown = Arc::new(AtomicBool::new(false));
        let response_delay_ms = Arc::new(AtomicU64::new(0));
        let active_requests = Arc::new(AtomicUsize::new(0));
        let max_concurrent_requests = Arc::new(AtomicUsize::new(0));
        let shutdown_flag = Arc::clone(&shutdown);
        let response_delay_ms_flag = Arc::clone(&response_delay_ms);
        let active_requests_flag = Arc::clone(&active_requests);
        let max_concurrent_requests_flag = Arc::clone(&max_concurrent_requests);
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let response_delay_ms_flag = Arc::clone(&response_delay_ms_flag);
                        let active_requests_flag = Arc::clone(&active_requests_flag);
                        let max_concurrent_requests_flag =
                            Arc::clone(&max_concurrent_requests_flag);
                        thread::spawn(move || {
                            handle_usage_request(
                                stream,
                                &response_delay_ms_flag,
                                &active_requests_flag,
                                &max_concurrent_requests_flag,
                            );
                        });
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            listen_addr,
            shutdown,
            response_delay_ms,
            max_concurrent_requests,
            thread: Some(thread),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}/backend-api", self.listen_addr)
    }

    fn set_delay_ms(&self, delay_ms: u64) {
        self.response_delay_ms.store(delay_ms, Ordering::SeqCst);
        self.max_concurrent_requests.store(0, Ordering::SeqCst);
    }

    fn max_concurrent_requests(&self) -> usize {
        self.max_concurrent_requests.load(Ordering::SeqCst)
    }
}

impl Drop for UsageServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.listen_addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle_usage_request(
    mut stream: TcpStream,
    response_delay_ms: &AtomicU64,
    active_requests: &AtomicUsize,
    max_concurrent_requests: &AtomicUsize,
) {
    let request = match read_http_request(&mut stream) {
        Some(request) => request,
        None => return,
    };

    let concurrent_requests = active_requests.fetch_add(1, Ordering::SeqCst) + 1;
    max_concurrent_requests.fetch_max(concurrent_requests, Ordering::SeqCst);

    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let authorization = request_header(&request, "Authorization");
    let account_id = request_header(&request, "ChatGPT-Account-Id");

    let (status_line, body) =
        if !(path.ends_with("/backend-api/wham/usage") || path.ends_with("/api/codex/usage")) {
            (
                "HTTP/1.1 404 Not Found",
                json!({ "error": "not_found" }).to_string(),
            )
        } else {
            match (authorization.as_deref(), account_id.as_deref()) {
                (Some("Bearer test-token"), Some("main-account")) => {
                    ("HTTP/1.1 200 OK", main_usage_body())
                }
                (Some("Bearer test-token"), Some("second-account")) => {
                    ("HTTP/1.1 200 OK", second_usage_body())
                }
                (Some("Bearer test-token"), Some("third-account")) => {
                    ("HTTP/1.1 200 OK", third_usage_body())
                }
                (Some("Bearer test-token"), Some("elite-account")) => {
                    ("HTTP/1.1 200 OK", elite_usage_body())
                }
                _ => (
                    "HTTP/1.1 401 Unauthorized",
                    json!({ "error": "unauthorized" }).to_string(),
                ),
            }
        };

    let delay_ms = response_delay_ms.load(Ordering::SeqCst);
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }

    let response = format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    active_requests.fetch_sub(1, Ordering::SeqCst);
}

fn read_http_request(stream: &mut TcpStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let mut buffer = [0_u8; 1024];
    let mut request = Vec::new();

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(_) => return None,
        }
    }

    if request.is_empty() {
        return None;
    }

    Some(String::from_utf8_lossy(&request).into_owned())
}

fn request_header(request: &str, header_name: &str) -> Option<String> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case(header_name) {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

fn future_epoch(offset_seconds: i64) -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
        + offset_seconds
}

fn main_usage_body() -> String {
    json!({
        "email": "main@example.com",
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 100,
                "reset_at": future_epoch(1_800),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 20,
                "reset_at": future_epoch(432_000),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

fn second_usage_body() -> String {
    json!({
        "email": "second@example.com",
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 20,
                "reset_at": future_epoch(7_200),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 30,
                "reset_at": future_epoch(518_400),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

fn third_usage_body() -> String {
    json!({
        "email": "third@example.com",
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 40,
                "reset_at": future_epoch(14_400),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 10,
                "reset_at": future_epoch(259_200),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

fn elite_usage_body() -> String {
    json!({
        "email": "elite@example.com",
        "plan_type": "team",
        "rate_limit": {
            "primary_window": {
                "used_percent": 1,
                "reset_at": future_epoch(3_600),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 1,
                "reset_at": future_epoch(172_800),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

struct Fixture {
    _temp_dir: TestDir,
    usage_server: UsageServer,
    usage_base_url: String,
    prodex_home: PathBuf,
    shared_codex_home: PathBuf,
    main_home: PathBuf,
    second_home: PathBuf,
    codex_log: PathBuf,
    codex_args_log: PathBuf,
    codex_stdin_log: PathBuf,
    codex_bin: PathBuf,
}

fn setup_fixture() -> Fixture {
    let temp_dir = TestDir::new();
    let usage_server = UsageServer::start();
    let usage_base_url = usage_server.base_url();
    let prodex_home = temp_dir.path.join("prodex-home");
    let shared_codex_home = temp_dir.path.join("shared-codex-home");
    let homes_root = temp_dir.path.join("homes");
    let bin_root = temp_dir.path.join("bin");
    let main_home = homes_root.join("main");
    let second_home = homes_root.join("second");
    let codex_log = temp_dir.path.join("codex-home.log");
    let codex_args_log = temp_dir.path.join("codex-args.log");
    let codex_stdin_log = temp_dir.path.join("codex-stdin.log");
    let codex_bin = bin_root.join("codex");

    fs::create_dir_all(&prodex_home).expect("failed to create prodex home");
    fs::create_dir_all(&shared_codex_home).expect("failed to create shared codex home");
    fs::create_dir_all(&main_home).expect("failed to create main home");
    fs::create_dir_all(&second_home).expect("failed to create second home");
    fs::create_dir_all(&bin_root).expect("failed to create bin dir");

    write_json(
        &prodex_home.join("state.json"),
        &json!({
            "active_profile": "main",
            "profiles": {
                "main": {
                    "codex_home": main_home,
                    "managed": true
                },
                "second": {
                    "codex_home": second_home,
                    "managed": true
                }
            }
        }),
    );

    write_json(
        &main_home.join("auth.json"),
        &json!({
            "tokens": {
                "access_token": "test-token",
                "account_id": "main-account"
            }
        }),
    );
    write_json(
        &second_home.join("auth.json"),
        &json!({
            "tokens": {
                "access_token": "test-token",
                "account_id": "second-account"
            }
        }),
    );

    write_executable(
        &codex_bin,
        r#"#!/bin/sh
printf '%s\n' "$CODEX_HOME" > "$TEST_CODEX_LOG"
if [ -n "$TEST_CODEX_ARGS_LOG" ]; then
  : > "$TEST_CODEX_ARGS_LOG"
  for arg in "$@"; do
    printf '%s\n' "$arg" >> "$TEST_CODEX_ARGS_LOG"
  done
fi
if [ -n "$TEST_CODEX_STDIN_LOG" ]; then
  cat > "$TEST_CODEX_STDIN_LOG"
fi
if [ -n "$TEST_LONG_RUNNING_RUN" ]; then
  sleep "$TEST_LONG_RUNNING_RUN"
fi
if [ "$1" = "login" ]; then
  mkdir -p "$CODEX_HOME"
  account_id="${TEST_LOGIN_ACCOUNT_ID:-main-account}"
  token="${TEST_LOGIN_ACCESS_TOKEN:-test-token}"
  id_token="${TEST_LOGIN_ID_TOKEN:-}"
  if [ -n "$id_token" ]; then
    printf '{"tokens":{"id_token":"%s","access_token":"%s","account_id":"%s"}}\n' "$id_token" "$token" "$account_id" > "$CODEX_HOME/auth.json"
  else
    printf '{"tokens":{"access_token":"%s","account_id":"%s"}}\n' "$token" "$account_id" > "$CODEX_HOME/auth.json"
  fi
fi
session_marker="${TEST_SESSION_MARKER:-}"
if [ -n "$session_marker" ]; then
  mkdir -p "$CODEX_HOME/sessions"
  printf '%s\n' "$session_marker" >> "$CODEX_HOME/history.jsonl"
  printf '{"marker":"%s"}\n' "$session_marker" > "$CODEX_HOME/sessions/$session_marker.json"
fi
memory_marker="${TEST_MEMORY_MARKER:-}"
if [ -n "$memory_marker" ]; then
  mkdir -p "$CODEX_HOME/memories"
  printf '{"memory":"%s"}\n' "$memory_marker" > "$CODEX_HOME/memories/$memory_marker.json"
fi
exit 0
"#,
    );

    Fixture {
        _temp_dir: temp_dir,
        usage_server,
        usage_base_url,
        prodex_home,
        shared_codex_home,
        main_home,
        second_home,
        codex_log,
        codex_args_log,
        codex_stdin_log,
        codex_bin,
    }
}

fn write_json(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create json parent dir");
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("failed to encode json"),
    )
    .expect("failed to write json");
}

fn write_executable(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create executable parent dir");
    }
    fs::write(path, content).expect("failed to write executable");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(path, permissions).expect("failed to chmod executable");
    }
}

fn run_prodex(fixture: &Fixture, args: &[&str]) -> std::process::Output {
    run_prodex_with_env(fixture, args, &[])
}

fn run_prodex_with_env(
    fixture: &Fixture,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_prodex"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PRODEX_HOME", &fixture.prodex_home)
        .env("PRODEX_SHARED_CODEX_HOME", &fixture.shared_codex_home)
        .env("PRODEX_CODEX_BIN", &fixture.codex_bin)
        .env("CODEX_CHATGPT_BASE_URL", &fixture.usage_base_url)
        .env("TEST_CODEX_LOG", &fixture.codex_log)
        .env("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "30000")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS", "1500")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS", "3000")
        .env("PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS", "250")
        .envs(extra_env.iter().copied())
        .args(args)
        .output()
        .expect("failed to execute prodex")
}

fn run_prodex_with_env_and_stdin(
    fixture: &Fixture,
    args: &[&str],
    extra_env: &[(&str, &str)],
    stdin: &str,
) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PRODEX_HOME", &fixture.prodex_home)
        .env("PRODEX_SHARED_CODEX_HOME", &fixture.shared_codex_home)
        .env("PRODEX_CODEX_BIN", &fixture.codex_bin)
        .env("CODEX_CHATGPT_BASE_URL", &fixture.usage_base_url)
        .env("TEST_CODEX_LOG", &fixture.codex_log)
        .env("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "30000")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS", "1500")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS", "3000")
        .env("PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS", "250")
        .envs(extra_env.iter().copied())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn prodex");

    let mut child_stdin = child.stdin.take().expect("prodex stdin should be piped");
    child_stdin
        .write_all(stdin.as_bytes())
        .expect("failed to write prodex stdin");
    drop(child_stdin);

    child
        .wait_with_output()
        .expect("failed to wait for prodex output")
}

fn spawn_prodex_with_env(fixture: &Fixture, args: &[&str], extra_env: &[(&str, &str)]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_prodex"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PRODEX_HOME", &fixture.prodex_home)
        .env("PRODEX_SHARED_CODEX_HOME", &fixture.shared_codex_home)
        .env("PRODEX_CODEX_BIN", &fixture.codex_bin)
        .env("CODEX_CHATGPT_BASE_URL", &fixture.usage_base_url)
        .env("TEST_CODEX_LOG", &fixture.codex_log)
        .env("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "30000")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS", "1500")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS", "3000")
        .env("PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS", "250")
        .envs(extra_env.iter().copied())
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .expect("failed to spawn prodex")
}

fn read_state(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path.join("state.json")).expect("failed to read state.json"))
        .expect("failed to parse state.json")
}

fn read_access_token(codex_home: &Path) -> String {
    serde_json::from_slice::<Value>(
        &fs::read(codex_home.join("auth.json")).expect("failed to read auth.json"),
    )
    .expect("failed to parse auth.json")["tokens"]["access_token"]
        .as_str()
        .expect("access_token should be a string")
        .to_string()
}

fn active_profile(path: &Path) -> String {
    read_state(path)["active_profile"]
        .as_str()
        .expect("active_profile should be a string")
        .to_string()
}

fn runtime_broker_registry_path(prodex_home: &Path) -> Option<PathBuf> {
    fs::read_dir(prodex_home)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("runtime-broker-") && name.ends_with(".json"))
        })
}

fn wait_for_runtime_broker_registry_path(prodex_home: &Path) -> PathBuf {
    test_wait::wait_for_poll(
        "runtime broker registry",
        Duration::from_secs(30),
        Duration::from_millis(10),
        || runtime_broker_registry_path(prodex_home),
    )
}

fn add_managed_profile(fixture: &Fixture, name: &str, account_id: &str) -> PathBuf {
    let home = fixture.prodex_home.join(format!("{name}-home"));
    fs::create_dir_all(&home).expect("failed to create additional home");
    write_json(
        &home.join("auth.json"),
        &json!({
            "tokens": {
                "access_token": "test-token",
                "account_id": account_id
            }
        }),
    );

    let mut state = read_state(&fixture.prodex_home);
    let profiles = state
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .expect("profiles should be an object");
    profiles.insert(
        name.to_string(),
        json!({
            "codex_home": home,
            "managed": true
        }),
    );
    write_json(&fixture.prodex_home.join("state.json"), &state);

    fixture.prodex_home.join(format!("{name}-home"))
}

fn chatgpt_id_token(email: &str) -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(format!(r#"{{"email":"{email}"}}"#));
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("sig");
    format!("{header}.{payload}.{signature}")
}

#[path = "auto_rotate/login.rs"]
mod login;
#[path = "auto_rotate/quota_doctor.rs"]
mod quota_doctor;
#[path = "auto_rotate/run.rs"]
mod run;
#[path = "auto_rotate/shared_state.rs"]
mod shared_state;
