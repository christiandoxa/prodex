use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct DashboardChild {
    child: Child,
}

impl Drop for DashboardChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn dashboard_control_plane_endpoints_work_and_redact_secrets() {
    let root = temp_root("dashboard-control-plane");
    fs::create_dir_all(&root).expect("temp root should be created");
    let runtime_logs = root.join("runtime-logs");
    let shared_codex = root.join("shared-codex");
    fs::create_dir_all(&runtime_logs).expect("runtime log root should be created");
    fs::create_dir_all(&shared_codex).expect("shared codex root should be created");
    let runtime_log = runtime_logs.join("prodex-runtime-dashboard-test.log");
    fs::write(
        &runtime_log,
        "runtime_ready route=responses\nauthorization=Bearer sk-secret-dashboard-test\n",
    )
    .expect("runtime log should be written");
    fs::write(
        runtime_logs.join("prodex-runtime-latest.path"),
        format!("{}\n", runtime_log.display()),
    )
    .expect("runtime log pointer should be written");

    let port = free_port();
    let server = spawn_dashboard(&root, &runtime_logs, &shared_codex, port);

    let state = wait_for_json(port, "/api/state");
    assert_eq!(state["profileCount"], 0);
    assert!(state["activeProfile"].is_null());

    let health = get_json(port, "/healthz");
    assert_eq!(health["status"], "ok");
    assert_eq!(health["service"], "prodex-dashboard");

    let logs = get_json(port, "/api/logs");
    assert!(logs["lines"].as_array().unwrap().len() >= 2);
    let logs_text = serde_json::to_string(&logs).unwrap();
    assert!(logs_text.contains("runtime_ready"));
    assert!(!logs_text.contains("sk-secret-dashboard-test"));
    assert!(logs_text.contains("<redacted>"));

    let (dashboard_head, dashboard_html) =
        get_response(port, "/").expect("dashboard shell should respond");
    assert!(
        dashboard_head
            .to_ascii_lowercase()
            .contains("content-security-policy:")
    );
    assert!(dashboard_html.contains("Prodex Control Center"));
    assert!(dashboard_html.contains("data-route=\"logs\""));
    assert!(!dashboard_html.contains("innerHTML"));

    let notices = get_text(port, "/third-party-notices")
        .expect("third-party notices endpoint should respond");
    assert!(notices.contains("OpenCodex"));
    assert!(notices.contains("MIT License"));

    let usage = get_json(port, "/api/usage");
    assert_eq!(usage["summary"]["total"], 0);

    let providers = get_json(port, "/api/provider-presets");
    assert_provider_ids(&providers["providers"]);
    let deepseek = provider(&providers, "deepseek");
    assert!(
        deepseek["commands"]["setup"]
            .as_array()
            .unwrap()
            .iter()
            .any(|cmd| cmd == "DEEPSEEK_API_KEY=... prodex s deepseek --model deepseek-v4-pro")
    );
    assert_eq!(
        deepseek["commands"]["launch"],
        "prodex s deepseek --model deepseek-v4-pro"
    );

    let provider_contracts = get_json(port, "/api/providers");
    assert_provider_ids(&provider_contracts["providers"]);
    assert!(provider_contracts["contracts"].as_array().unwrap().len() >= 7);

    let models = get_json(port, "/api/models");
    let model_rows = models["models"].as_array().unwrap();
    for id in [
        "openai",
        "gemini",
        "anthropic",
        "copilot",
        "deepseek",
        "kiro",
        "local",
    ] {
        assert!(
            model_rows.iter().any(|row| row["provider"] == id),
            "missing model rows for {id}"
        );
    }
    assert!(model_rows.iter().any(|row| row["recommended"] == true));

    let runtime = get_json(port, "/api/runtime-status");
    assert_eq!(
        runtime["runtime"]["doctorCommand"],
        "prodex doctor --runtime"
    );
    assert_eq!(
        runtime["gateway"]["providersCommand"],
        "prodex gateway providers --json"
    );

    write_secret_state(&root);
    for endpoint in [
        "/api/state",
        "/api/accounts",
        "/api/usage",
        "/api/providers",
        "/api/provider-presets",
        "/api/models",
        "/api/runtime-status",
        "/api/logs",
    ] {
        let body = get_text(port, endpoint).unwrap_or_else(|err| panic!("{endpoint}: {err}"));
        assert!(
            !body.contains("sk-secret-dashboard-test"),
            "{endpoint} leaked secret"
        );
    }
    let accounts_text = get_text(port, "/api/accounts").expect("accounts endpoint should respond");
    assert!(accounts_text.contains("openai-main"));

    drop(server);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dashboard_can_fall_back_when_the_requested_gui_port_is_busy() {
    let occupied = TcpListener::bind("127.0.0.1:0").expect("occupied port should bind");
    let port = occupied.local_addr().unwrap().port();
    let root = temp_root("dashboard-port-fallback");
    let runtime_logs = root.join("runtime-logs");
    let shared_codex = root.join("shared-codex");
    fs::create_dir_all(&runtime_logs).expect("runtime log root should be created");
    fs::create_dir_all(&shared_codex).expect("shared codex root should be created");

    let mut child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args([
            "dashboard",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--fallback-port",
        ])
        .env("PRODEX_HOME", &root)
        .env("PRODEX_RUNTIME_LOG_DIR", &runtime_logs)
        .env("PRODEX_SHARED_CODEX_HOME", &shared_codex)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("dashboard should spawn");

    thread::sleep(Duration::from_millis(500));
    assert!(
        child
            .try_wait()
            .expect("child status should be readable")
            .is_none(),
        "dashboard should remain running on its fallback port"
    );

    let server = DashboardChild { child };
    drop(server);
    drop(occupied);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(target_os = "linux")]
#[test]
fn gui_shortcut_opens_xdg_browser_and_keeps_serving() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("dashboard-linux-gui");
    let runtime_logs = root.join("runtime-logs");
    let shared_codex = root.join("shared-codex");
    let bin_dir = root.join("bin");
    let opener_output = root.join("opened-url.txt");
    fs::create_dir_all(&runtime_logs).expect("runtime log root should be created");
    fs::create_dir_all(&shared_codex).expect("shared codex root should be created");
    fs::create_dir_all(&bin_dir).expect("test bin directory should be created");
    let opener = bin_dir.join("xdg-open");
    fs::write(
        &opener,
        "#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$PRODEX_GUI_TEST_OUTPUT\"\n",
    )
    .expect("mock xdg-open should be written");
    fs::set_permissions(&opener, fs::Permissions::from_mode(0o755))
        .expect("mock xdg-open should be executable");

    let port = free_port();
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args(["gui", "--host", "127.0.0.1", "--port", &port.to_string()])
        .env("PATH", path)
        .env("PRODEX_HOME", &root)
        .env("PRODEX_RUNTIME_LOG_DIR", &runtime_logs)
        .env("PRODEX_SHARED_CODEX_HOME", &shared_codex)
        .env("PRODEX_GUI_TEST_OUTPUT", &opener_output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("GUI shortcut should spawn");
    let server = DashboardChild { child };

    let health = wait_for_json(port, "/healthz");
    assert_eq!(health["status"], "ok");
    for _ in 0..80 {
        if let Ok(url) = fs::read_to_string(&opener_output) {
            assert_eq!(url.trim(), format!("http://127.0.0.1:{port}"));
            drop(server);
            let _ = fs::remove_dir_all(&root);
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }

    panic!("GUI shortcut did not invoke xdg-open");
}

fn spawn_dashboard(
    root: &Path,
    runtime_logs: &Path,
    shared_codex: &Path,
    port: u16,
) -> DashboardChild {
    let child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args([
            "dashboard",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .env("PRODEX_HOME", root)
        .env("PRODEX_RUNTIME_LOG_DIR", runtime_logs)
        .env("PRODEX_SHARED_CODEX_HOME", shared_codex)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("dashboard should spawn");
    DashboardChild { child }
}

fn wait_for_json(port: u16, path: &str) -> Value {
    for _ in 0..80 {
        if let Ok(value) = try_get_json(port, path) {
            return value;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("dashboard did not serve {path}");
}

fn get_json(port: u16, path: &str) -> Value {
    try_get_json(port, path).expect("dashboard JSON endpoint should respond")
}

fn try_get_json(port: u16, path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&get_text(port, path)?)?)
}

fn get_text(port: u16, path: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(get_response(port, path)?.1)
}

fn get_response(port: u16, path: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or("invalid HTTP response")?;
    assert!(head.contains(" 200 "), "unexpected response head: {head}");
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        return Ok((head.to_string(), decode_chunked(body)?));
    }
    Ok((head.to_string(), body.to_string()))
}

fn decode_chunked(body: &str) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = body.as_bytes();
    let mut index = 0usize;
    let mut decoded = Vec::new();
    loop {
        let Some(line_end) = find_crlf(bytes, index) else {
            return Err("invalid chunked response".into());
        };
        let size_text = std::str::from_utf8(&bytes[index..line_end])?;
        let size = usize::from_str_radix(size_text.trim(), 16)?;
        index = line_end + 2;
        if size == 0 {
            break;
        }
        if index + size > bytes.len() {
            return Err("truncated chunked response".into());
        }
        decoded.extend_from_slice(&bytes[index..index + size]);
        index += size;
        if bytes.get(index..index + 2) != Some(b"\r\n") {
            return Err("invalid chunk terminator".into());
        }
        index += 2;
    }
    Ok(String::from_utf8(decoded)?)
}

fn find_crlf(bytes: &[u8], from: usize) -> Option<usize> {
    bytes[from..]
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|offset| from + offset)
}

fn assert_provider_ids(providers: &Value) {
    let ids = providers
        .as_array()
        .unwrap()
        .iter()
        .map(|provider| provider["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    for expected in [
        "openai",
        "gemini",
        "anthropic",
        "copilot",
        "deepseek",
        "kiro",
        "local",
    ] {
        assert!(ids.contains(&expected), "missing provider {expected}");
    }
}

fn provider<'a>(providers: &'a Value, id: &str) -> &'a Value {
    providers["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == id)
        .unwrap_or_else(|| panic!("missing provider {id}"))
}

fn write_secret_state(root: &Path) {
    let profile_home = root.join("profiles/openai-main");
    fs::create_dir_all(&profile_home).expect("profile home should be created");
    fs::write(
        profile_home.join("auth.json"),
        r#"{"not_auth_secret":"sk-secret-dashboard-test"}"#,
    )
    .expect("auth file should be written");
    let state = serde_json::json!({
        "active_profile": "openai-main",
        "profiles": {
            "openai-main": {
                "codex_home": profile_home,
                "managed": true,
                "email": "user@example.com",
                "provider": { "provider_kind": "openai" }
            }
        }
    });
    fs::write(
        root.join("state.json"),
        serde_json::to_string_pretty(&state).unwrap(),
    )
    .expect("state file should be written");
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("free port should bind");
    listener.local_addr().unwrap().port()
}

fn temp_root(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("prodex-{name}-{}-{nanos}", std::process::id()))
}
