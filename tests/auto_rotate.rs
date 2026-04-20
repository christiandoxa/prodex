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
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    let started_at = Instant::now();
    loop {
        if let Some(path) = runtime_broker_registry_path(prodex_home) {
            return path;
        }
        assert!(
            started_at.elapsed() < Duration::from_secs(30),
            "timed out waiting for runtime broker registry"
        );
        thread::sleep(Duration::from_millis(10));
    }
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

#[test]
fn run_auto_rotates_active_profile_when_current_is_blocked() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Quota preflight blocked profile 'main'"));
    assert!(stderr.contains("profile 'second'"));
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
}

#[test]
fn explicit_profile_auto_rotates_by_default() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run", "--profile", "main"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Auto-rotating to profile 'second'."));
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
}

#[test]
fn run_exec_preserves_prompt_and_piped_stdin() {
    let fixture = setup_fixture();
    let args_log = fixture.codex_args_log.display().to_string();
    let stdin_log = fixture.codex_stdin_log.display().to_string();

    let output = run_prodex_with_env_and_stdin(
        &fixture,
        &[
            "run",
            "--profile",
            "main",
            "--no-auto-rotate",
            "--skip-quota-check",
            "exec",
            "summarize concisely",
        ],
        &[
            ("TEST_CODEX_ARGS_LOG", args_log.as_str()),
            ("TEST_CODEX_STDIN_LOG", stdin_log.as_str()),
        ],
        "piped input\n",
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(active_profile(&fixture.prodex_home), "main");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.main_home.display().to_string()
    );
    assert_eq!(
        fs::read_to_string(&fixture.codex_args_log)
            .expect("failed to read codex args log")
            .lines()
            .collect::<Vec<_>>(),
        vec!["exec", "summarize concisely"]
    );
    assert_eq!(
        fs::read_to_string(&fixture.codex_stdin_log).expect("failed to read codex stdin log"),
        "piped input\n"
    );
}

#[test]
fn explicit_profile_can_disable_auto_rotate() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run", "--profile", "main", "--no-auto-rotate"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Other profiles that look ready: second")
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Rerun without `--no-auto-rotate` to allow fallback.")
    );
    assert_eq!(active_profile(&fixture.prodex_home), "main");
    assert!(!fixture.codex_log.exists());
    assert_eq!(
        fixture.main_home.file_name().and_then(|name| name.to_str()),
        Some("main")
    );
}

#[test]
fn run_preflight_checks_fallback_profiles_in_parallel() {
    let fixture = setup_fixture();
    add_managed_profile(&fixture, "third", "third-account");
    fixture.usage_server.set_delay_ms(80);

    let output = run_prodex(&fixture, &["run", "--profile", "main", "--no-auto-rotate"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Other profiles that look ready: third, second")
    );
    assert!(
        fixture.usage_server.max_concurrent_requests() >= 2,
        "fallback profile checks never overlapped"
    );
}

#[test]
fn run_without_profile_keeps_the_active_ready_account() {
    let fixture = setup_fixture();
    add_managed_profile(&fixture, "elite", "elite-account");
    let mut state = read_state(&fixture.prodex_home);
    state["active_profile"] = Value::String("second".to_string());
    write_json(&fixture.prodex_home.join("state.json"), &state);

    let output = run_prodex(&fixture, &["run"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Auto-selecting profile"));
}

#[cfg(unix)]
#[test]
fn run_recovers_when_runtime_broker_registry_points_to_a_dead_pid() {
    let fixture = setup_fixture();
    let mut child = spawn_prodex_with_env(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
        &[("TEST_LONG_RUNNING_RUN", "5")],
    );

    let registry_path = wait_for_runtime_broker_registry_path(&fixture.prodex_home);
    let initial_registry: Value =
        serde_json::from_slice(&fs::read(&registry_path).expect("failed to read registry"))
            .expect("failed to parse registry");
    let initial_pid = initial_registry["pid"]
        .as_u64()
        .expect("registry pid should be numeric") as u32;
    assert!(
        initial_pid > 0,
        "registry should contain a live broker pid before failure"
    );

    let _ = Command::new("kill")
        .arg("-9")
        .arg(initial_pid.to_string())
        .status()
        .expect("failed to kill broker pid");

    let _ = child.kill();
    let _ = child.wait();

    let output = run_prodex(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let recovered_registry: Value = serde_json::from_slice(
        &fs::read(&registry_path).expect("failed to read recovered registry"),
    )
    .expect("failed to parse recovered registry");
    let recovered_pid = recovered_registry["pid"]
        .as_u64()
        .expect("recovered registry pid should be numeric") as u32;
    assert_ne!(
        recovered_pid, initial_pid,
        "a stale broker registry should be replaced by a fresh broker process"
    );
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.main_home.display().to_string()
    );
}

#[test]
fn doctor_quota_checks_profiles_in_parallel() {
    let fixture = setup_fixture();
    fixture.usage_server.set_delay_ms(80);

    let output = run_prodex(&fixture, &["doctor", "--quota"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fixture.usage_server.max_concurrent_requests() >= 2,
        "doctor quota checks never overlapped"
    );
}

#[test]
fn quota_raw_uses_builtin_usage_client() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["quota", "--profile", "second", "--raw"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let usage: Value =
        serde_json::from_slice(&output.stdout).expect("failed to parse raw quota output");
    assert_eq!(usage["email"], "second@example.com");
    assert_eq!(usage["plan_type"], "plus");
    assert_eq!(
        usage["rate_limit"]["secondary_window"]["limit_window_seconds"],
        604_800
    );
}

#[test]
fn quota_all_detail_shows_main_reset_times() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["quota", "--all", "--detail", "--once"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Quota Overview"));
    assert!(stdout.contains("REMAINING"));
    assert!(stdout.contains("status: Blocked: 5h exhausted until "));
    assert!(stdout.contains("status: Ready"));
    assert!(stdout.contains("resets: 5h "));
    assert!(stdout.contains("| weekly "));
}

#[test]
fn quota_all_detail_sorts_by_status_then_nearest_main_reset() {
    let fixture = setup_fixture();
    add_managed_profile(&fixture, "third", "third-account");

    let output = run_prodex(&fixture, &["quota", "--all", "--detail", "--once"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let profile_lines = stdout
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("second") {
                Some(("second", index))
            } else if trimmed.starts_with("third") {
                Some(("third", index))
            } else if trimmed.starts_with("main") {
                Some(("main", index))
            } else {
                None
            }
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let second_index = *profile_lines
        .get("second")
        .expect("second profile should be rendered");
    let third_index = *profile_lines
        .get("third")
        .expect("third profile should be rendered");
    let main_index = *profile_lines
        .get("main")
        .expect("main profile should be rendered");

    assert!(
        second_index < third_index,
        "ready profile with sooner reset should sort first"
    );
    assert!(
        third_index < main_index,
        "blocked profiles should sort after ready profiles"
    );
}

#[test]
fn run_shares_resume_history_across_managed_profiles() {
    let fixture = setup_fixture();
    let seeded_session_dir = fixture.main_home.join("sessions/2026/03");
    fs::create_dir_all(&seeded_session_dir).expect("failed to create seeded session dir");
    fs::write(fixture.main_home.join("history.jsonl"), "seed-main\n")
        .expect("failed to seed history");
    fs::write(seeded_session_dir.join("seed.json"), "{\"seed\":true}\n")
        .expect("failed to seed session");

    let first_output = run_prodex_with_env(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
        &[("TEST_SESSION_MARKER", "main-run")],
    );
    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    let second_output = run_prodex_with_env(
        &fixture,
        &["run", "--profile", "second", "--skip-quota-check"],
        &[("TEST_SESSION_MARKER", "second-run")],
    );
    assert!(
        second_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );

    let main_history = fs::read_to_string(fixture.main_home.join("history.jsonl"))
        .expect("failed to read main history");
    let second_history = fs::read_to_string(fixture.second_home.join("history.jsonl"))
        .expect("failed to read second history");
    assert_eq!(main_history, second_history);
    assert!(main_history.contains("seed-main"));
    assert!(main_history.contains("main-run"));
    assert!(main_history.contains("second-run"));

    assert!(
        fixture
            .main_home
            .join("sessions/2026/03/seed.json")
            .is_file()
    );
    assert!(
        fixture
            .second_home
            .join("sessions/2026/03/seed.json")
            .is_file()
    );
    assert!(fixture.main_home.join("sessions/main-run.json").is_file());
    assert!(fixture.second_home.join("sessions/main-run.json").is_file());
    assert!(fixture.main_home.join("sessions/second-run.json").is_file());
    assert!(
        fixture
            .second_home
            .join("sessions/second-run.json")
            .is_file()
    );

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_link(fixture.main_home.join("history.jsonl"))
                .expect("failed to read main history link"),
            fixture.shared_codex_home.join("history.jsonl")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("history.jsonl"))
                .expect("failed to read second history link"),
            fixture.shared_codex_home.join("history.jsonl")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("sessions"))
                .expect("failed to read main sessions link"),
            fixture.shared_codex_home.join("sessions")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("sessions"))
                .expect("failed to read second sessions link"),
            fixture.shared_codex_home.join("sessions")
        );
        assert!(
            fs::symlink_metadata(fixture.main_home.join("history.jsonl"))
                .expect("failed to inspect main history")
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(fixture.second_home.join("history.jsonl"))
                .expect("failed to inspect second history")
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(fixture.main_home.join("sessions"))
                .expect("failed to inspect main sessions")
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(fixture.second_home.join("sessions"))
                .expect("failed to inspect second sessions")
                .file_type()
                .is_symlink()
        );
    }
}

#[test]
fn run_shares_housekeeping_memories_across_managed_profiles() {
    let fixture = setup_fixture();
    fs::create_dir_all(fixture.main_home.join("memories")).expect("failed to create memories dir");
    fs::write(
        fixture.main_home.join("memories/seed-memory.json"),
        "{\"seed\":true}\n",
    )
    .expect("failed to seed memory");

    let first_output = run_prodex_with_env(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
        &[("TEST_MEMORY_MARKER", "main-memory")],
    );
    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    let second_output = run_prodex_with_env(
        &fixture,
        &["run", "--profile", "second", "--skip-quota-check"],
        &[("TEST_MEMORY_MARKER", "second-memory")],
    );
    assert!(
        second_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );

    for home in [&fixture.main_home, &fixture.second_home] {
        assert!(home.join("memories/seed-memory.json").is_file());
        assert!(home.join("memories/main-memory.json").is_file());
        assert!(home.join("memories/second-memory.json").is_file());
    }

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_link(fixture.main_home.join("memories"))
                .expect("failed to read main memories link"),
            fixture.shared_codex_home.join("memories")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("memories"))
                .expect("failed to read second memories link"),
            fixture.shared_codex_home.join("memories")
        );
        assert!(
            fs::symlink_metadata(fixture.main_home.join("memories"))
                .expect("failed to inspect main memories")
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::symlink_metadata(fixture.second_home.join("memories"))
                .expect("failed to inspect second memories")
                .file_type()
                .is_symlink()
        );
    }
}

#[test]
fn run_shares_native_codex_behavior_state_across_managed_profiles() {
    let fixture = setup_fixture();
    fs::write(
        fixture.main_home.join("config.toml"),
        "model = \"gpt-5.4\"\nmodel_reasoning_effort = \"xhigh\"\n",
    )
    .expect("failed to seed config");
    fs::create_dir_all(fixture.main_home.join("rules")).expect("failed to create main rules dir");
    fs::write(
        fixture.main_home.join("rules/default.rules"),
        "main-rule = true\n",
    )
    .expect("failed to seed main rule");
    fs::create_dir_all(fixture.main_home.join("skills/main-skill"))
        .expect("failed to create main skill dir");
    fs::write(
        fixture.main_home.join("skills/main-skill/SKILL.md"),
        "# Main Skill\n",
    )
    .expect("failed to seed main skill");

    let first_output = run_prodex(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
    );
    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    fs::create_dir_all(fixture.second_home.join("rules"))
        .expect("failed to create second rules dir");
    fs::write(
        fixture.second_home.join("rules/team.rules"),
        "second-rule = true\n",
    )
    .expect("failed to seed second rule");
    fs::create_dir_all(fixture.second_home.join("skills/second-skill"))
        .expect("failed to create second skill dir");
    fs::write(
        fixture.second_home.join("skills/second-skill/SKILL.md"),
        "# Second Skill\n",
    )
    .expect("failed to seed second skill");

    let second_output = run_prodex(
        &fixture,
        &["run", "--profile", "second", "--skip-quota-check"],
    );
    assert!(
        second_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );

    for home in [&fixture.main_home, &fixture.second_home] {
        let config = fs::read_to_string(home.join("config.toml"))
            .expect("shared config.toml should be readable");
        assert!(config.contains("model_reasoning_effort = \"xhigh\""));
        assert!(home.join("rules/default.rules").is_file());
        assert!(home.join("rules/team.rules").is_file());
        assert!(home.join("skills/main-skill/SKILL.md").is_file());
        assert!(home.join("skills/second-skill/SKILL.md").is_file());
    }

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_link(fixture.main_home.join("config.toml"))
                .expect("failed to read main config link"),
            fixture.shared_codex_home.join("config.toml")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("config.toml"))
                .expect("failed to read second config link"),
            fixture.shared_codex_home.join("config.toml")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("rules")).expect("failed to read main rules link"),
            fixture.shared_codex_home.join("rules")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("rules"))
                .expect("failed to read second rules link"),
            fixture.shared_codex_home.join("rules")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("skills"))
                .expect("failed to read main skills link"),
            fixture.shared_codex_home.join("skills")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("skills"))
                .expect("failed to read second skills link"),
            fixture.shared_codex_home.join("skills")
        );
    }
}

#[test]
fn run_shares_codex_v121_marketplaces_plugins_and_memory_extensions_across_managed_profiles() {
    let fixture = setup_fixture();
    fs::write(
        fixture.main_home.join("config.toml"),
        r#"[features]
plugins = true

[marketplaces.debug]
source_type = "git"
source = "https://github.com/example/debug-marketplace.git"
last_updated = "2026-04-16T00:00:00Z"

[plugins."sample-plugin@debug"]
enabled = true
"#,
    )
    .expect("failed to seed marketplace config");

    fs::create_dir_all(
        fixture
            .main_home
            .join(".tmp/marketplaces/debug/.agents/plugins"),
    )
    .expect("failed to create main marketplace manifest dir");
    write_json(
        &fixture
            .main_home
            .join(".tmp/marketplaces/debug/.agents/plugins/marketplace.json"),
        &json!({
            "name": "debug",
            "plugins": [
                {
                    "name": "sample-plugin",
                    "source": {
                        "type": "local",
                        "path": "./plugins/sample-plugin"
                    },
                    "policy": {
                        "installation": "AVAILABLE",
                        "authentication": "ON_INSTALL"
                    }
                }
            ]
        }),
    );
    fs::create_dir_all(
        fixture
            .main_home
            .join(".tmp/marketplaces/debug/plugins/sample-plugin/.codex-plugin"),
    )
    .expect("failed to create main marketplace plugin dir");
    write_json(
        &fixture
            .main_home
            .join(".tmp/marketplaces/debug/plugins/sample-plugin/.codex-plugin/plugin.json"),
        &json!({
            "name": "sample-plugin",
            "version": "local"
        }),
    );
    fs::write(
        fixture
            .main_home
            .join(".tmp/marketplaces/debug/plugins/sample-plugin/marketplace-main.txt"),
        "main marketplace marker\n",
    )
    .expect("failed to seed main marketplace marker");

    fs::create_dir_all(
        fixture
            .main_home
            .join("plugins/cache/debug/sample-plugin/local/.codex-plugin"),
    )
    .expect("failed to create main plugin cache dir");
    write_json(
        &fixture
            .main_home
            .join("plugins/cache/debug/sample-plugin/local/.codex-plugin/plugin.json"),
        &json!({
            "name": "sample-plugin",
            "version": "local"
        }),
    );
    fs::write(
        fixture
            .main_home
            .join("plugins/cache/debug/sample-plugin/local/plugin-main.txt"),
        "main plugin marker\n",
    )
    .expect("failed to seed main plugin marker");

    fs::create_dir_all(fixture.main_home.join("memories_extensions/team/resources"))
        .expect("failed to create main memories extension dir");
    fs::write(
        fixture
            .main_home
            .join("memories_extensions/team/instructions.md"),
        "# Team memory extension\n",
    )
    .expect("failed to seed extension instructions");
    fs::write(
        fixture
            .main_home
            .join("memories_extensions/team/resources/main.txt"),
        "main extension marker\n",
    )
    .expect("failed to seed main extension marker");

    let first_output = run_prodex(
        &fixture,
        &["run", "--profile", "main", "--skip-quota-check"],
    );
    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );

    fs::create_dir_all(
        fixture
            .second_home
            .join("plugins/cache/debug/sample-plugin/1.2.3/.codex-plugin"),
    )
    .expect("failed to create second plugin cache dir");
    write_json(
        &fixture
            .second_home
            .join("plugins/cache/debug/sample-plugin/1.2.3/.codex-plugin/plugin.json"),
        &json!({
            "name": "sample-plugin",
            "version": "1.2.3"
        }),
    );
    fs::write(
        fixture
            .second_home
            .join("plugins/cache/debug/sample-plugin/1.2.3/plugin-second.txt"),
        "second plugin marker\n",
    )
    .expect("failed to seed second plugin marker");

    fs::create_dir_all(
        fixture
            .second_home
            .join("memories_extensions/team/resources"),
    )
    .expect("failed to create second memories extension dir");
    fs::write(
        fixture
            .second_home
            .join("memories_extensions/team/resources/second.txt"),
        "second extension marker\n",
    )
    .expect("failed to seed second extension marker");

    let second_output = run_prodex(
        &fixture,
        &["run", "--profile", "second", "--skip-quota-check"],
    );
    assert!(
        second_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );

    for home in [&fixture.main_home, &fixture.second_home] {
        let config = fs::read_to_string(home.join("config.toml"))
            .expect("shared config.toml should be readable");
        assert!(config.contains("[marketplaces.debug]"));
        assert!(config.contains("[plugins.\"sample-plugin@debug\"]"));
        assert!(
            home.join(".tmp/marketplaces/debug/.agents/plugins/marketplace.json")
                .is_file()
        );
        assert!(
            home.join(".tmp/marketplaces/debug/plugins/sample-plugin/marketplace-main.txt")
                .is_file()
        );
        assert!(
            home.join("plugins/cache/debug/sample-plugin/local/plugin-main.txt")
                .is_file()
        );
        assert!(
            home.join("plugins/cache/debug/sample-plugin/1.2.3/plugin-second.txt")
                .is_file()
        );
        assert!(
            home.join("memories_extensions/team/instructions.md")
                .is_file()
        );
        assert!(
            home.join("memories_extensions/team/resources/main.txt")
                .is_file()
        );
        assert!(
            home.join("memories_extensions/team/resources/second.txt")
                .is_file()
        );
    }

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_link(fixture.main_home.join("plugins"))
                .expect("failed to read main plugins link"),
            fixture.shared_codex_home.join("plugins")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("plugins"))
                .expect("failed to read second plugins link"),
            fixture.shared_codex_home.join("plugins")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join("memories_extensions"))
                .expect("failed to read main memories_extensions link"),
            fixture.shared_codex_home.join("memories_extensions")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join("memories_extensions"))
                .expect("failed to read second memories_extensions link"),
            fixture.shared_codex_home.join("memories_extensions")
        );
        assert_eq!(
            fs::read_link(fixture.main_home.join(".tmp/marketplaces"))
                .expect("failed to read main marketplaces link"),
            fixture.shared_codex_home.join(".tmp/marketplaces")
        );
        assert_eq!(
            fs::read_link(fixture.second_home.join(".tmp/marketplaces"))
                .expect("failed to read second marketplaces link"),
            fixture.shared_codex_home.join(".tmp/marketplaces")
        );
    }
}

#[test]
fn login_without_profile_creates_profile_from_email() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "profiles": {}
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "main_example.com");
    assert_eq!(
        state["profiles"]["main_example.com"]["email"],
        "main@example.com"
    );
    assert_eq!(
        state["profiles"].as_object().map(|profiles| profiles.len()),
        Some(1)
    );
    assert!(
        state["profiles"]["main_example.com"]["codex_home"]
            .as_str()
            .expect("codex_home should be a string")
            .ends_with("/profiles/main_example.com")
    );
    assert!(
        fixture
            .prodex_home
            .join("profiles/main_example.com/auth.json")
            .is_file()
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Logged in as main@example.com. Created profile 'main_example.com'.")
    );
}

#[test]
fn login_without_profile_reuses_existing_profile_for_same_email() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "active_profile": "primary",
            "profiles": {
                "primary": {
                    "codex_home": fixture.main_home,
                    "managed": true
                }
            }
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "primary");
    assert_eq!(state["profiles"]["primary"]["email"], "main@example.com");
    assert_eq!(
        state["profiles"].as_object().map(|profiles| profiles.len()),
        Some(1)
    );
    assert!(!fixture.prodex_home.join("profiles/primary").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        "Logged in as main@example.com. Updated auth token for existing profile 'primary'."
    ));
}

#[test]
fn login_without_profile_updates_token_only_for_duplicate_email() {
    let fixture = setup_fixture();
    let id_token = chatgpt_id_token("main@example.com");
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "active_profile": "primary",
            "profiles": {
                "primary": {
                    "codex_home": fixture.main_home,
                    "managed": true
                }
            }
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[
            ("TEST_LOGIN_ACCOUNT_ID", "main-account"),
            ("TEST_LOGIN_ACCESS_TOKEN", "fresh-token"),
            ("TEST_LOGIN_ID_TOKEN", id_token.as_str()),
            ("TEST_SESSION_MARKER", "duplicate-login"),
        ],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let auth_json = fs::read_to_string(fixture.main_home.join("auth.json"))
        .expect("updated auth.json should exist");
    assert_eq!(
        serde_json::from_str::<Value>(&auth_json)
            .expect("auth.json should parse")["tokens"]["access_token"]
            .as_str(),
        Some("fresh-token")
    );
    assert!(
        !fixture
            .shared_codex_home
            .join("sessions/duplicate-login.json")
            .exists(),
        "duplicate login should not copy session state into the existing profile"
    );
}

#[test]
fn login_without_profile_looks_up_existing_profiles_in_parallel() {
    let fixture = setup_fixture();
    add_managed_profile(&fixture, "third", "third-account");
    fixture.usage_server.set_delay_ms(80);

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        "Logged in as main@example.com. Updated auth token for existing profile 'main'."
    ));
    assert!(
        fixture.usage_server.max_concurrent_requests() >= 2,
        "login profile lookup never overlapped"
    );
}

#[test]
fn login_without_profile_adds_suffix_when_email_name_is_taken() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "active_profile": "main_example.com",
            "profiles": {
                "main_example.com": {
                    "codex_home": fixture.second_home,
                    "managed": true,
                    "email": "second@example.com"
                }
            }
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[("TEST_LOGIN_ACCOUNT_ID", "main-account")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "main_example.com-2");
    assert_eq!(
        state["profiles"]["main_example.com-2"]["email"],
        "main@example.com"
    );
    assert_eq!(
        state["profiles"]["main_example.com"]["email"],
        "second@example.com"
    );
    assert!(
        fixture
            .prodex_home
            .join("profiles/main_example.com-2/auth.json")
            .is_file()
    );
}

#[test]
fn login_without_profile_uses_auth_email_before_quota_lookup() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "profiles": {}
        }),
    );
    let id_token = chatgpt_id_token("token@example.com");

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[
            ("TEST_LOGIN_ACCOUNT_ID", "main-account"),
            ("TEST_LOGIN_ID_TOKEN", id_token.as_str()),
            ("CODEX_CHATGPT_BASE_URL", "http://127.0.0.1:1"),
        ],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "token_example.com");
    assert_eq!(
        state["profiles"]["token_example.com"]["email"],
        "token@example.com"
    );
}

#[test]
fn login_without_profile_falls_back_to_usage_email_when_id_token_is_missing() {
    let fixture = setup_fixture();
    write_json(
        &fixture.prodex_home.join("state.json"),
        &json!({
            "profiles": {}
        }),
    );

    let output = run_prodex_with_env(
        &fixture,
        &["login"],
        &[
            ("TEST_LOGIN_ACCOUNT_ID", "main-account"),
            ("CODEX_CHATGPT_BASE_URL", fixture.usage_base_url.as_str()),
        ],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state = read_state(&fixture.prodex_home);
    assert_eq!(state["active_profile"], "main_example.com");
    assert_eq!(
        state["profiles"]["main_example.com"]["email"],
        "main@example.com"
    );

    let auth_json = fs::read_to_string(
        fixture
            .prodex_home
            .join("profiles/main_example.com/auth.json"),
    )
    .expect("failed to read created auth.json");
    assert!(
        !auth_json.contains("\"id_token\""),
        "auth.json should not contain an id_token when login falls back to usage email"
    );
}
