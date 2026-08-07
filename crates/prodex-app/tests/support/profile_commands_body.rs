use crate::TestEnvVarGuard;
use base64::Engine as _;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

#[path = "profile_commands_body/export.rs"]
mod export;
#[path = "profile_commands_body/import.rs"]
mod import;
#[path = "profile_commands_body/lifecycle.rs"]
mod lifecycle;
#[path = "profile_commands_body/login.rs"]
mod login;

#[test]
fn profile_add_and_remove_succeed_when_local_audit_persistence_fails() {
    let root = ProfileCommandsTestDir::new("audit-best-effort");
    let _env = ProfileCommandsTestEnv::new(&root.path);
    let audit_blocker = root.path.join("audit-blocker");
    fs::write(&audit_blocker, "blocked").expect("audit blocker should be written");
    let _audit_guard =
        TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", &audit_blocker.display().to_string());

    handle_add_profile(AddProfileArgs {
        name: "main".to_string(),
        codex_home: None,
        copy_from: None,
        copy_current: false,
        activate: true,
        insecure: false,
    })
    .expect("profile add should not fail after its state commit");

    let paths = AppPaths::discover().expect("paths should resolve");
    assert!(
        AppState::load(&paths)
            .expect("state should load")
            .profiles
            .contains_key("main")
    );

    handle_remove_profile(RemoveProfileArgs {
        name: Some("main".to_string()),
        all: false,
        delete_home: false,
    })
    .expect("profile remove should not fail after its state commit");

    assert!(
        !AppState::load(&paths)
            .expect("state should load")
            .profiles
            .contains_key("main")
    );
}

#[test]
fn profile_remove_recovers_quarantined_home_after_state_save_failure() {
    let root = ProfileCommandsTestDir::new("remove-lifecycle-recovery");
    let _env = ProfileCommandsTestEnv::new(&root.path);
    let paths = AppPaths::discover().expect("paths should resolve");
    let profile_home = paths.managed_profiles_root.join("main");
    profile_commands_write_profile_auth(&profile_home, "main");
    AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("initial state should save");

    {
        let _fault = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE", "1");
        handle_remove_profile(RemoveProfileArgs {
            name: Some("main".to_string()),
            all: false,
            delete_home: true,
        })
        .expect_err("remove should fail when its state commit fails");
    }

    assert!(
        AppState::load(&paths)
            .expect("state should remain readable")
            .profiles
            .contains_key("main"),
        "failed remove must leave the durable profile state unchanged"
    );
    assert!(
        !profile_home.exists(),
        "failed remove should leave the home quarantined"
    );
    assert!(
        prodex_profile_export::profile_lifecycle_journal_paths(&paths.root)
            .expect("lifecycle journal should be readable")
            .len()
            == 1
    );

    handle_remove_profile(RemoveProfileArgs {
        name: Some("main".to_string()),
        all: false,
        delete_home: true,
    })
    .expect("retry should recover and complete remove");

    assert!(
        !profile_home.exists(),
        "completed remove should delete the home"
    );
    assert!(
        !AppState::load(&paths)
            .expect("state should load")
            .profiles
            .contains_key("main")
    );
    assert!(
        prodex_profile_export::profile_lifecycle_journal_paths(&paths.root)
            .expect("lifecycle journals should be readable")
            .is_empty()
    );
}

#[cfg(unix)]
#[test]
fn profile_remove_recovers_and_deletes_dangling_home_symlink() {
    use std::os::unix::fs::symlink;

    let root = ProfileCommandsTestDir::new("remove-dangling-home");
    let _env = ProfileCommandsTestEnv::new(&root.path);
    let paths = AppPaths::discover().expect("paths should resolve");
    let profile_home = paths.managed_profiles_root.join("main");
    symlink(paths.root.join("missing-home-target"), &profile_home)
        .expect("dangling profile home symlink should be created");
    AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("initial state should save");

    {
        let _fault = TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_STATE_SAVE_ERROR_ONCE", "1");
        handle_remove_profile(RemoveProfileArgs {
            name: Some("main".to_string()),
            all: false,
            delete_home: true,
        })
        .expect_err("remove should fail when its state commit fails");
    }
    assert!(
        fs::symlink_metadata(&profile_home).is_err(),
        "failed remove should quarantine the dangling symlink"
    );

    handle_remove_profile(RemoveProfileArgs {
        name: Some("main".to_string()),
        all: false,
        delete_home: true,
    })
    .expect("retry should recover and complete remove");
    assert!(
        fs::symlink_metadata(&profile_home).is_err(),
        "completed remove must not leave a dangling home symlink"
    );
}

#[test]
fn profile_remove_recovers_after_continuation_sidecar_failure() {
    let root = ProfileCommandsTestDir::new("remove-sidecar-recovery");
    let _env = ProfileCommandsTestEnv::new(&root.path);
    let paths = AppPaths::discover().expect("paths should resolve");
    let profile_home = paths.managed_profiles_root.join("main");
    profile_commands_write_profile_auth(&profile_home, "main");
    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    state.save(&paths).expect("initial state should save");
    save_runtime_continuations_for_profiles(
        &paths,
        &RuntimeContinuationStore::default(),
        &state.profiles,
    )
    .expect("continuation sidecar should save");

    {
        let _fault =
            TestEnvVarGuard::set("PRODEX_RUNTIME_FAULT_CONTINUATIONS_SAVE_ERROR_ONCE", "1");
        handle_remove_profile(RemoveProfileArgs {
            name: Some("main".to_string()),
            all: false,
            delete_home: true,
        })
        .expect_err("remove should fail when sidecar persistence fails");
    }

    assert!(
        !AppState::load(&paths)
            .expect("state should load")
            .profiles
            .contains_key("main"),
        "state commit should remain durable before sidecar retry"
    );
    assert!(
        !profile_home.exists(),
        "home should remain quarantined for retry"
    );

    let runtime_log_dir = root.path.join("runtime-logs");
    fs::create_dir_all(&runtime_log_dir).expect("isolated runtime log directory should exist");
    let _runtime_log_guard = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        &runtime_log_dir.display().to_string(),
    );
    crate::command_dispatch::execute_command(crate::Commands::Cleanup(CleanupArgs::default()))
        .expect("cleanup should finalize committed remove recovery");

    assert!(!profile_home.exists());
    assert!(
        prodex_profile_export::profile_lifecycle_journal_paths(&paths.root)
            .expect("lifecycle journals should be readable")
            .is_empty()
    );
    assert!(
        !fs::read_to_string(runtime_continuations_file_path(&paths))
            .expect("continuation sidecar should remain readable")
            .contains("main")
    );
    let journal_path = runtime_continuation_journal_file_path(&paths);
    if journal_path.exists() {
        assert!(
            !fs::read_to_string(journal_path)
                .expect("continuation journal should remain readable")
                .contains("main")
        );
    }
}

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
        profile_commands_create_private_test_dir(&path);
        Self { path }
    }
}

impl Drop for ProfileCommandsTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct ProfileCommandsTestEnv {
    _copilot_home_guard: TestEnvVarGuard,
    _kiro_bin_guard: TestEnvVarGuard,
    _kiro_db_guard: TestEnvVarGuard,
    _prodex_guard: TestEnvVarGuard,
    _shared_override_guard: TestEnvVarGuard,
    _home_guards: (TestEnvVarGuard, TestEnvVarGuard),
}

impl ProfileCommandsTestEnv {
    fn new(root: &Path) -> Self {
        let home = root.join("home");
        let prodex_home = root.join("prodex");
        profile_commands_create_private_test_dir(&home);
        profile_commands_create_private_test_dir(&prodex_home);
        profile_commands_create_private_test_dir(&prodex_home.join("profiles"));
        Self {
            // Acquire the outer env lock first; this field is declared last so it drops last.
            _home_guards: TestEnvVarGuard::set_home(&home),
            _copilot_home_guard: TestEnvVarGuard::set(
                "COPILOT_HOME",
                &home.join(".copilot").display().to_string(),
            ),
            _kiro_bin_guard: TestEnvVarGuard::set(
                "PRODEX_KIRO_BIN",
                &root.join("missing-kiro-cli").display().to_string(),
            ),
            _kiro_db_guard: TestEnvVarGuard::set(
                "KIRO_TEST_DB_PATH",
                &home.join(".local/share/kiro-cli/data.sqlite3").display().to_string(),
            ),
            _prodex_guard: TestEnvVarGuard::set("PRODEX_HOME", &prodex_home.display().to_string()),
            _shared_override_guard: TestEnvVarGuard::unset("PRODEX_SHARED_CODEX_HOME"),
        }
    }
}

fn profile_commands_test_paths(root: &Path) -> AppPaths {
    let paths = AppPaths {
        root: root.to_path_buf(),
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join(".codex"),
        legacy_shared_codex_root: root.join("shared"),
    };
    profile_commands_create_private_test_dir(&paths.managed_profiles_root);
    paths
}

fn profile_commands_create_private_test_dir(path: &Path) {
    create_codex_home_if_missing(path).expect("private test directory should be created");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o700,
            "{} should be a trusted private test parent",
            path.display()
        );
    }
}

#[cfg(unix)]
fn profile_commands_replace_test_dir_with_symlink(path: &Path, target: &Path) {
    fs::remove_dir(path).expect("test directory should be removable");
    fs::create_dir_all(target).expect("symlink target should exist");
    std::os::unix::fs::symlink(target, path).expect("test directory symlink should be created");
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
        auth_json["tokens"]["refresh_token"] = serde_json::Value::String(refresh_token.to_string());
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
    wake_addr: std::net::SocketAddr,
    handle: Option<JoinHandle<()>>,
}

impl ProfileCommandsOneShotHttpServer {
    fn start_json(body: serde_json::Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server should bind");
        let wake_addr = listener
            .local_addr()
            .expect("server address should resolve");
        let base_url = format!("http://{wake_addr}");
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
            wake_addr,
            handle: Some(handle),
        }
    }
}

impl Drop for ProfileCommandsOneShotHttpServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = std::net::TcpStream::connect(self.wake_addr);
            let _ = handle.join();
        }
    }
}
