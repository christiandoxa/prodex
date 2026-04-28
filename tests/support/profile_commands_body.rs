use crate::TestEnvVarGuard;
use base64::Engine as _;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

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
        fs::create_dir_all(&path).expect("test dir should be created");
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
    serde_json::json!({
        "auth_mode": "chatgpt",
        "tokens": {
            "access_token": access_token,
            "account_id": account_id,
            "id_token": profile_commands_id_token(email)
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

fn profile_commands_read_access_token(codex_home: &Path) -> String {
    serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(codex_home.join("auth.json")).expect("auth.json should be readable"),
    )
    .expect("auth.json should parse")["tokens"]["access_token"]
        .as_str()
        .expect("access token should be a string")
        .to_string()
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

#[test]
fn profile_export_round_trip_plain_imports_profiles_and_sets_active() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let source_dir = ProfileCommandsTestDir::new("export-source");
    let source_paths = profile_commands_test_paths(&source_dir.path);
    let main_home = source_paths.managed_profiles_root.join("main");
    let second_home = source_paths.managed_profiles_root.join("second");
    profile_commands_write_profile_auth(&main_home, "main");
    profile_commands_write_profile_auth(&second_home, "second");

    let source_state = AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: false,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        ..AppState::default()
    };

    let selected_names = vec!["main".to_string(), "second".to_string()];
    let payload =
        build_profile_export_payload(&source_state, &selected_names).expect("payload builds");
    let encoded =
        serialize_profile_export_payload(&payload, None).expect("plain export should encode");
    let decoded = decode_profile_export_envelope(
        serde_json::from_slice(&encoded).expect("encoded bundle should parse"),
    )
    .expect("plain export should decode");

    let target_dir = ProfileCommandsTestDir::new("export-target");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let mut target_state = AppState::default();
    let commit = import_profile_export_payload(&target_paths, &mut target_state, &decoded)
        .expect("import should succeed");

    assert_eq!(
        commit.imported_names,
        vec!["main".to_string(), "second".to_string()]
    );
    assert_eq!(target_state.active_profile.as_deref(), Some("second"));

    let imported_main = target_state
        .profiles
        .get("main")
        .expect("main profile should exist");
    assert!(imported_main.managed);
    assert_eq!(imported_main.email.as_deref(), Some("main@example.com"));
    assert_eq!(
        fs::read_to_string(imported_main.codex_home.join("auth.json"))
            .expect("imported auth should exist"),
        profile_commands_sample_auth_json("main")
    );

    let imported_second = target_state
        .profiles
        .get("second")
        .expect("second profile should exist");
    assert!(imported_second.managed);
    assert_eq!(imported_second.email.as_deref(), Some("second@example.com"));
    assert_eq!(
        fs::read_to_string(imported_second.codex_home.join("auth.json"))
            .expect("imported auth should exist"),
        profile_commands_sample_auth_json("second")
    );
}

#[test]
fn profile_export_round_trip_encrypted_requires_matching_password() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("main".to_string()),
        profiles: vec![ExportedProfile {
            name: "main".to_string(),
            email: Some("main@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Openai,
            auth_json: profile_commands_sample_auth_json("main"),
        }],
    };

    let encoded = serialize_profile_export_payload(&payload, Some("secret-password"))
        .expect("encrypted export should encode");
    let envelope: ProfileExportEnvelope =
        serde_json::from_slice(&encoded).expect("encrypted bundle should parse");
    let decoded = match envelope {
        ProfileExportEnvelope::Encrypted {
            format,
            version,
            cipher,
            kdf,
            iterations,
            salt_base64,
            nonce_base64,
            ciphertext_base64,
        } => {
            validate_profile_export_header(&format, version).expect("header should validate");
            assert_eq!(cipher, PROFILE_EXPORT_CIPHER);
            assert_eq!(kdf, PROFILE_EXPORT_KDF);
            let salt = base64::engine::general_purpose::STANDARD
                .decode(salt_base64)
                .expect("salt should decode");
            let nonce = base64::engine::general_purpose::STANDARD
                .decode(nonce_base64)
                .expect("nonce should decode");
            let ciphertext = base64::engine::general_purpose::STANDARD
                .decode(ciphertext_base64)
                .expect("ciphertext should decode");
            let key = derive_profile_export_key("secret-password", &salt, iterations);
            let cipher = Aes256GcmSiv::new_from_slice(&key).expect("cipher should init");
            let plaintext = cipher
                .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
                .expect("ciphertext should decrypt");
            serde_json::from_slice::<ProfileExportPayload>(&plaintext)
                .expect("payload should parse")
        }
        _ => panic!("expected encrypted bundle"),
    };

    assert_eq!(decoded.active_profile.as_deref(), Some("main"));
    assert_eq!(decoded.profiles.len(), 1);
    assert_eq!(
        decoded.profiles[0].auth_json,
        profile_commands_sample_auth_json("main")
    );
}

#[cfg(unix)]
#[test]
fn profile_export_bundle_is_written_with_private_permissions() {
    use super::import_export::write_profile_export_bundle;
    use std::os::unix::fs::PermissionsExt;

    let target_dir = ProfileCommandsTestDir::new("export-bundle-permissions");
    let output_path = target_dir.path.join("bundle.json");

    write_profile_export_bundle(&output_path, b"{\"secret\":true}")
        .expect("profile export bundle should be written");

    let mode = fs::metadata(&output_path)
        .expect("profile export bundle should exist")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn profile_import_copilot_reads_provider_metadata_from_logged_in_cli_state() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let copilot_home = sandbox_dir.path.join("home/.copilot");
    fs::create_dir_all(&copilot_home).expect("copilot home should be created");

    let server = ProfileCommandsOneShotHttpServer::start_json(serde_json::json!({
        "login": "copilot-user",
        "access_type_sku": "copilot_standalone_seat_quota",
        "copilot_plan": "business",
        "endpoints": {
            "api": "https://api.example.githubcopilot.test"
        }
    }));
    let host = server.base_url.clone();
    let account_key = format!("{host}:copilot-user");
    fs::write(
        copilot_home.join("config.json"),
        serde_json::json!({
            "lastLoggedInUser": {
                "host": host,
                "login": "copilot-user"
            },
            "copilotTokens": {
                account_key: "copilot-token"
            }
        })
        .to_string(),
    )
    .expect("copilot config should be written");

    handle_import_profiles(ImportProfileArgs {
        path: PathBuf::from("copilot"),
        name: Some("copilot-main".to_string()),
        activate: true,
    })
    .expect("copilot import should succeed");

    let paths = AppPaths::discover().expect("paths should resolve");
    let state = AppState::load(&paths).expect("state should load");
    let profile = state
        .profiles
        .get("copilot-main")
        .expect("copilot profile should exist");
    assert!(profile.managed);
    assert_eq!(state.active_profile.as_deref(), Some("copilot-main"));
    assert_eq!(profile.email.as_deref(), Some("copilot-user"));
    assert!(profile.codex_home.exists());
    match &profile.provider {
        ProfileProvider::Copilot {
            host,
            login,
            api_url,
            access_type_sku,
            copilot_plan,
        } => {
            assert_eq!(host, &server.base_url);
            assert_eq!(login, "copilot-user");
            assert_eq!(api_url, "https://api.example.githubcopilot.test");
            assert_eq!(
                access_type_sku.as_deref(),
                Some("copilot_standalone_seat_quota")
            );
            assert_eq!(copilot_plan.as_deref(), Some("business"));
        }
        other => panic!("expected copilot provider, got {other:?}"),
    }
}

#[test]
fn copilot_quota_lookup_reads_provider_quota_for_the_selected_account() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let copilot_home = sandbox_dir.path.join("home/.copilot");
    fs::create_dir_all(&copilot_home).expect("copilot home should be created");

    let server = ProfileCommandsOneShotHttpServer::start_json(serde_json::json!({
        "login": "copilot-user",
        "access_type_sku": "free_limited_copilot",
        "copilot_plan": "individual",
        "limited_user_quotas": {
            "chat": 450,
            "completions": 4000
        },
        "monthly_quotas": {
            "chat": 500,
            "completions": 4000
        },
        "limited_user_reset_date": "2026-05-09",
        "endpoints": {
            "api": "https://api.individual.githubcopilot.com"
        }
    }));
    let host = server.base_url.clone();
    let account_key = format!("{host}:copilot-user");
    fs::write(
        copilot_home.join("config.json"),
        serde_json::json!({
            "lastLoggedInUser": {
                "host": host,
                "login": "copilot-user"
            },
            "copilotTokens": {
                account_key: "copilot-token"
            }
        })
        .to_string(),
    )
    .expect("copilot config should be written");

    let info = fetch_copilot_user_info_for_account(&server.base_url, "copilot-user")
        .expect("copilot quota lookup should succeed");

    assert_eq!(info.login.as_deref(), Some("copilot-user"));
    assert_eq!(info.copilot_plan.as_deref(), Some("individual"));
    assert_eq!(info.access_type_sku.as_deref(), Some("free_limited_copilot"));
    assert_eq!(info.limited_user_quotas.get("chat").copied(), Some(450));
    assert_eq!(info.monthly_quotas.get("chat").copied(), Some(500));
    assert_eq!(info.limited_user_reset_date.as_deref(), Some("2026-05-09"));
}

#[test]
fn profile_export_round_trip_preserves_copilot_provider_metadata() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let source_dir = ProfileCommandsTestDir::new("copilot-export-source");
    let source_paths = profile_commands_test_paths(&source_dir.path);
    let copilot_home = source_paths.managed_profiles_root.join("copilot-main");
    create_codex_home_if_missing(&copilot_home).expect("copilot home should exist");

    let source_state = AppState {
        active_profile: Some("copilot-main".to_string()),
        profiles: BTreeMap::from([(
            "copilot-main".to_string(),
            ProfileEntry {
                codex_home: copilot_home,
                managed: true,
                email: Some("copilot-user".to_string()),
                provider: ProfileProvider::Copilot {
                    host: "https://github.com".to_string(),
                    login: "copilot-user".to_string(),
                    api_url: "https://api.example.githubcopilot.test".to_string(),
                    access_type_sku: Some("copilot_standalone_seat_quota".to_string()),
                    copilot_plan: Some("business".to_string()),
                },
            },
        )]),
        ..AppState::default()
    };

    let payload = build_profile_export_payload(&source_state, &["copilot-main".to_string()])
        .expect("payload should build");
    assert_eq!(payload.profiles.len(), 1);
    assert!(payload.profiles[0].auth_json.is_empty());

    let target_dir = ProfileCommandsTestDir::new("copilot-export-target");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let mut target_state = AppState::default();
    import_profile_export_payload(&target_paths, &mut target_state, &payload)
        .expect("copilot import should succeed");

    let imported = target_state
        .profiles
        .get("copilot-main")
        .expect("copilot profile should be imported");
    assert_eq!(imported.email.as_deref(), Some("copilot-user"));
    assert!(!imported.codex_home.join("auth.json").exists());
    match &imported.provider {
        ProfileProvider::Copilot {
            host,
            login,
            api_url,
            access_type_sku,
            copilot_plan,
        } => {
            assert_eq!(host, "https://github.com");
            assert_eq!(login, "copilot-user");
            assert_eq!(api_url, "https://api.example.githubcopilot.test");
            assert_eq!(
                access_type_sku.as_deref(),
                Some("copilot_standalone_seat_quota")
            );
            assert_eq!(copilot_plan.as_deref(), Some("business"));
        }
        other => panic!("expected copilot provider, got {other:?}"),
    }
}

#[test]
fn profile_import_updates_existing_profile_when_name_matches() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-collision");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");

    let mut existing_state = AppState {
        active_profile: None,
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: existing_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("main".to_string()),
        profiles: vec![ExportedProfile {
            name: "main".to_string(),
            email: Some("imported@example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Openai,
            auth_json: profile_commands_auth_json_with_email(
                "imported@example.com",
                "fresh-token",
                "imported-account",
            ),
        }],
    };

    let commit = import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("import should update same-name profile");

    assert!(commit.imported_names.is_empty());
    assert_eq!(commit.updated_existing_names, vec!["main".to_string()]);
    assert_eq!(existing_state.active_profile.as_deref(), Some("main"));
    assert_eq!(existing_state.profiles.len(), 1);
    assert_eq!(
        existing_state
            .profiles
            .get("main")
            .and_then(|profile| profile.email.as_deref()),
        Some("imported@example.com")
    );
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "fresh-token".to_string()
    );
    assert_eq!(
        fs::read_dir(&target_paths.managed_profiles_root)
            .expect("managed root should be readable")
            .count(),
        1,
        "same-name import should not create a new managed home"
    );
}

#[test]
fn profile_import_updates_existing_profile_when_email_matches() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-duplicate-email");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("primary");
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");

    let mut existing_state = AppState {
        profiles: BTreeMap::from([(
            "primary".to_string(),
            ProfileEntry {
                codex_home: existing_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };
    let payload = ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: Some("backup-main".to_string()),
        profiles: vec![ExportedProfile {
            name: "backup-main".to_string(),
            email: Some("Main@Example.com".to_string()),
            source_managed: true,
            provider: ProfileProvider::Openai,
            auth_json: profile_commands_auth_json_with_email(
                "main@example.com",
                "fresh-token",
                "main-account",
            ),
        }],
    };

    let commit = import_profile_export_payload(&target_paths, &mut existing_state, &payload)
        .expect("import should update existing duplicate");

    assert!(commit.imported_names.is_empty());
    assert_eq!(commit.updated_existing_names, vec!["primary".to_string()]);
    assert_eq!(existing_state.active_profile.as_deref(), Some("primary"));
    assert_eq!(existing_state.profiles.len(), 1);
    assert_eq!(
        existing_state
            .profiles
            .get("primary")
            .and_then(|profile| profile.email.as_deref()),
        Some("main@example.com")
    );
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "fresh-token".to_string()
    );
    assert!(
        !target_paths
            .managed_profiles_root
            .join("backup-main")
            .exists(),
        "duplicate import should not create a new managed home"
    );
}

#[test]
fn import_current_updates_existing_profile_token_for_duplicate_email() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let paths = AppPaths::discover().expect("app paths should resolve");
    let existing_home = paths.managed_profiles_root.join("primary");
    let current_home = paths.shared_codex_root.clone();
    create_codex_home_if_missing(&existing_home).expect("existing home should exist");
    create_codex_home_if_missing(&current_home).expect("current home should exist");
    write_secret_text_file(
        &existing_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "old-token", "main-account"),
    )
    .expect("existing auth should be written");
    write_secret_text_file(
        &current_home.join("auth.json"),
        &profile_commands_auth_json_with_email("main@example.com", "new-token", "main-account"),
    )
    .expect("current auth should be written");

    AppState {
        active_profile: None,
        profiles: BTreeMap::from([(
            "primary".to_string(),
            ProfileEntry {
                codex_home: existing_home.clone(),
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("state should save");

    handle_import_current_profile(ImportCurrentArgs {
        name: "duplicate".to_string(),
    })
    .expect("import-current should update duplicate auth");

    let state = AppState::load(&paths).expect("state should load");
    assert_eq!(state.active_profile.as_deref(), Some("primary"));
    assert_eq!(state.profiles.len(), 1);
    assert_eq!(
        state
            .profiles
            .get("primary")
            .and_then(|profile| profile.email.as_deref()),
        Some("main@example.com")
    );
    assert_eq!(
        profile_commands_read_access_token(&existing_home),
        "new-token".to_string()
    );
    assert!(
        !paths.managed_profiles_root.join("duplicate").exists(),
        "duplicate import-current should not create a new managed home"
    );
}
