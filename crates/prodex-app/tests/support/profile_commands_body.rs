use crate::TestEnvVarGuard;
use base64::Engine as _;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

#[path = "profile_commands_body/export.rs"]
mod export;
#[path = "profile_commands_body/import.rs"]
mod import;
#[path = "profile_commands_body/login.rs"]
mod login;

#[test]
fn current_profile_auto_repairs_missing_active_profile() {
    let root = ProfileCommandsTestDir::new("current-repair-missing-active");
    let _env = ProfileCommandsTestEnv::new(&root.path);
    let paths = AppPaths::discover().expect("paths should resolve");
    fs::create_dir_all(&paths.root).expect("prodex home should exist");
    let profile_home = paths.managed_profiles_root.join("main");
    profile_commands_write_profile_auth(&profile_home, "main");
    let now = Local::now().timestamp();
    fs::write(
        &paths.state_file,
        serde_json::to_string_pretty(&serde_json::json!({
            "active_profile": "deleted",
            "profiles": {
                "main": {
                    "codex_home": profile_home,
                    "managed": true,
                    "email": null,
                    "provider_kind": "openai"
                }
            },
            "last_run_selected_at": {
                "deleted": now,
                "main": now
            },
            "response_profile_bindings": {
                "orphan": {
                    "profile_name": "deleted",
                    "bound_at": now
                }
            },
            "session_profile_bindings": {}
        }))
        .expect("state should render"),
    )
    .expect("state should be written");

    handle_current_profile().expect("current profile should auto-repair");

    let state = AppState::load(&paths).expect("state should load");
    assert_eq!(state.active_profile.as_deref(), Some("main"));
    assert_eq!(
        state.last_run_selected_at,
        BTreeMap::from([("main".to_string(), now)])
    );
    assert!(state.response_profile_bindings.is_empty());
}

#[test]
fn list_profiles_auto_selects_when_no_active_profile_exists() {
    let root = ProfileCommandsTestDir::new("list-repair-no-active");
    let _env = ProfileCommandsTestEnv::new(&root.path);
    let paths = AppPaths::discover().expect("paths should resolve");
    let profile_home = paths.managed_profiles_root.join("main");
    profile_commands_write_profile_auth(&profile_home, "main");
    AppState {
        active_profile: None,
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("state should save");

    handle_list_profiles().expect("list profiles should auto-select");

    let state = AppState::load(&paths).expect("state should load");
    assert_eq!(state.active_profile.as_deref(), Some("main"));
}

struct ProfileCommandsTestDir {
    path: PathBuf,
}

impl ProfileCommandsTestDir {
    fn new(prefix: &str) -> Self {
        let path = env::temp_dir().join(format!(
            "prodex-{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        create_codex_home_if_missing(&path).expect("test dir should be created");
        Self { path }
    }
}

impl Drop for ProfileCommandsTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct ProfileCommandsTestEnv {
    _home_guard: TestEnvVarGuard,
    _prodex_guard: TestEnvVarGuard,
    _shared_override_guard: TestEnvVarGuard,
}

impl ProfileCommandsTestEnv {
    fn new(root: &Path) -> Self {
        let home = root.join("home");
        let prodex_home = root.join("prodex");
        fs::create_dir_all(&home).expect("test home should be created");
        fs::create_dir_all(&prodex_home).expect("test prodex home should be created");
        Self {
            _home_guard: TestEnvVarGuard::set("HOME", &home.display().to_string()),
            _prodex_guard: TestEnvVarGuard::set("PRODEX_HOME", &prodex_home.display().to_string()),
            _shared_override_guard: TestEnvVarGuard::unset("PRODEX_SHARED_CODEX_HOME"),
        }
    }
}

fn profile_commands_test_paths(root: &Path) -> AppPaths {
    AppPaths {
        root: root.to_path_buf(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join(".codex"),
        legacy_shared_codex_root: root.join("shared"),
    }
}

fn profile_commands_sample_auth_json(profile_name: &str) -> String {
    serde_json::json!({
        "auth_mode": "chatgpt",
        "tokens": {
            "access_token": format!("access-{profile_name}"),
            "account_id": format!("account-{profile_name}"),
            "id_token": "header.payload.signature"
        }
    })
    .to_string()
}

fn profile_commands_id_token(email: &str) -> String {
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::json!({ "email": email }).to_string());
    format!("header.{payload}.signature")
}

fn profile_commands_auth_json_with_email(
    email: &str,
    access_token: &str,
    account_id: &str,
) -> String {
    profile_commands_auth_json_with_email_and_refresh(email, access_token, account_id, None)
}

fn profile_commands_auth_json_with_email_and_refresh(
    email: &str,
    access_token: &str,
    account_id: &str,
    refresh_token: Option<&str>,
) -> String {
    let mut auth_json = serde_json::json!({
        "auth_mode": "chatgpt",
        "tokens": {
            "access_token": access_token,
            "account_id": account_id,
            "id_token": profile_commands_id_token(email)
        }
    });
    if let Some(refresh_token) = refresh_token {
        auth_json["tokens"]["refresh_token"] =
            serde_json::Value::String(refresh_token.to_string());
    }
    auth_json.to_string()
}

fn profile_commands_read_auth_json(codex_home: &Path) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(codex_home.join("auth.json")).expect("auth.json should be readable"),
    )
    .expect("auth.json should parse")
}

fn profile_commands_read_access_token(codex_home: &Path) -> String {
    profile_commands_read_auth_json(codex_home)["tokens"]["access_token"]
        .as_str()
        .expect("access token should be a string")
        .to_string()
}

fn profile_commands_read_refresh_token(codex_home: &Path) -> String {
    profile_commands_read_auth_json(codex_home)["tokens"]["refresh_token"]
        .as_str()
        .expect("refresh token should be a string")
        .to_string()
}

fn profile_commands_auth_json_without_email(
    access_token: &str,
    account_id: &str,
    refresh_token: &str,
) -> String {
    serde_json::json!({
        "auth_mode": "chatgpt",
        "tokens": {
            "access_token": access_token,
            "account_id": account_id,
            "refresh_token": refresh_token
        }
    })
    .to_string()
}

fn profile_commands_write_profile_auth(codex_home: &Path, profile_name: &str) {
    create_codex_home_if_missing(codex_home).expect("profile home should exist");
    write_secret_text_file(
        &codex_home.join("auth.json"),
        &profile_commands_sample_auth_json(profile_name),
    )
    .expect("auth.json should be written");
}

fn profile_commands_import_auth_journal_paths(paths: &AppPaths) -> Vec<PathBuf> {
    let journal_root = prodex_profile_export::profile_import_auth_update_journal_root(&paths.root);
    let entries = match fs::read_dir(&journal_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(err) => panic!("journal root should be readable: {err}"),
    };
    let mut paths = entries
        .map(|entry| entry.expect("journal entry should be readable").path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

struct ProfileCommandsOneShotHttpServer {
    base_url: String,
    handle: Option<JoinHandle<()>>,
}

impl ProfileCommandsOneShotHttpServer {
    fn start_json(body: serde_json::Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let base_url = format!(
            "http://{}",
            listener
                .local_addr()
                .expect("server address should resolve")
        );
        let body = body.to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test server should accept");
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("test server should write response");
        });
        Self {
            base_url,
            handle: Some(handle),
        }
    }
}

impl Drop for ProfileCommandsOneShotHttpServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
