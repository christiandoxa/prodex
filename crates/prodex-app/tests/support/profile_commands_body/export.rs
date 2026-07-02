use super::*;
use crate::profile_commands::kiro::{
    KIRO_CREDENTIALS_FILE, KIRO_MODEL_CATALOG_FILE, parse_kiro_auth_secret_text,
    parse_kiro_model_catalog_text, read_kiro_auth_secret,
};

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
    let decoded = prodex_profile_export::decode_profile_export_envelope(
        serde_json::from_slice(&encoded).expect("encoded bundle should parse"),
        || unreachable!("plain export should not request a password"),
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
fn profile_export_round_trip_preserves_refresh_token() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let source_dir = ProfileCommandsTestDir::new("export-refresh-source");
    let source_paths = profile_commands_test_paths(&source_dir.path);
    let main_home = source_paths.managed_profiles_root.join("main");
    create_codex_home_if_missing(&main_home).expect("main home should exist");
    write_secret_text_file(
        &main_home.join("auth.json"),
        &profile_commands_auth_json_with_email_and_refresh(
            "main@example.com",
            "fresh-access-token",
            "main-account",
            Some("fresh-refresh-token"),
        ),
    )
    .expect("source auth should be written");

    let source_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    };

    let payload = build_profile_export_payload(&source_state, &["main".to_string()])
        .expect("payload with refresh token should build");
    let exported_auth = serde_json::from_str::<serde_json::Value>(&payload.profiles[0].auth_json)
        .expect("exported auth should parse");
    assert_eq!(
        exported_auth["tokens"]["refresh_token"].as_str(),
        Some("fresh-refresh-token")
    );

    let encoded =
        serialize_profile_export_payload(&payload, None).expect("plain export should encode");
    let decoded = prodex_profile_export::decode_profile_export_envelope(
        serde_json::from_slice(&encoded).expect("encoded bundle should parse"),
        || unreachable!("plain export should not request a password"),
    )
    .expect("plain export should decode");

    let target_dir = ProfileCommandsTestDir::new("export-refresh-target");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let mut target_state = AppState::default();
    import_profile_export_payload(&target_paths, &mut target_state, &decoded)
        .expect("import should preserve refresh token");

    let imported_home = &target_state
        .profiles
        .get("main")
        .expect("main profile should exist")
        .codex_home;
    assert_eq!(
        profile_commands_read_access_token(imported_home),
        "fresh-access-token"
    );
    assert_eq!(
        profile_commands_read_refresh_token(imported_home),
        "fresh-refresh-token"
    );
}

#[test]
fn profile_export_round_trip_preserves_oauth_provider_secret_files() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let source_dir = ProfileCommandsTestDir::new("provider-export-source");
    let source_paths = profile_commands_test_paths(&source_dir.path);
    let gemini_home = source_paths.managed_profiles_root.join("gemini-main");
    let anthropic_home = source_paths.managed_profiles_root.join("claude-main");
    create_codex_home_if_missing(&gemini_home).expect("Gemini home should exist");
    create_codex_home_if_missing(&anthropic_home).expect("Anthropic home should exist");

    let gemini_secret = serde_json::json!({
        "auth_mode": "gemini_oauth",
        "access_token": "gemini-access-token",
        "refresh_token": "gemini-refresh-token",
        "token_type": "Bearer",
        "scope": "https://www.googleapis.com/auth/cloud-platform",
        "expiry_date": 1900000000000_i64,
        "email": "gemini@example.com",
        "project_id": "gemini-project"
    })
    .to_string();
    let claude_secret = serde_json::json!({
        "claudeAiOauth": {
            "accessToken": "claude-access-token",
            "expiresAt": 1900000000001_i64,
            "subscriptionType": "max",
            "email": "claude@example.com"
        }
    })
    .to_string();
    write_secret_text_file(&gemini_home.join(GEMINI_OAUTH_SECRET_FILE), &gemini_secret)
        .expect("Gemini secret should be written");
    write_secret_text_file(&anthropic_home.join(CLAUDE_CREDENTIALS_FILE), &claude_secret)
        .expect("Claude secret should be written");

    let source_state = AppState {
        active_profile: Some("gemini-main".to_string()),
        profiles: BTreeMap::from([
            (
                "gemini-main".to_string(),
                ProfileEntry {
                    codex_home: gemini_home,
                    managed: true,
                    email: Some("gemini@example.com".to_string()),
                    provider: ProfileProvider::Gemini {
                        email: "gemini@example.com".to_string(),
                        project_id: Some("gemini-project".to_string()),
                    },
                },
            ),
            (
                "claude-main".to_string(),
                ProfileEntry {
                    codex_home: anthropic_home,
                    managed: true,
                    email: Some("claude@example.com".to_string()),
                    provider: ProfileProvider::Anthropic {
                        account: Some("claude@example.com".to_string()),
                        auth_method: Some("claude-ai-oauth:max".to_string()),
                    },
                },
            ),
        ]),
        ..AppState::default()
    };

    let selected_names = vec!["gemini-main".to_string(), "claude-main".to_string()];
    let payload = build_profile_export_payload(&source_state, &selected_names)
        .expect("provider payload should build");
    assert_eq!(payload.profiles[0].auth_json, "");
    assert_eq!(payload.profiles[0].secret_files[0].path, GEMINI_OAUTH_SECRET_FILE);
    assert_eq!(payload.profiles[1].auth_json, "");
    assert_eq!(payload.profiles[1].secret_files[0].path, CLAUDE_CREDENTIALS_FILE);

    let encoded =
        serialize_profile_export_payload(&payload, None).expect("plain export should encode");
    let decoded = prodex_profile_export::decode_profile_export_envelope(
        serde_json::from_slice(&encoded).expect("encoded bundle should parse"),
        || unreachable!("plain export should not request a password"),
    )
    .expect("plain export should decode");

    let target_dir = ProfileCommandsTestDir::new("provider-export-target");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let mut target_state = AppState::default();
    import_profile_export_payload(&target_paths, &mut target_state, &decoded)
        .expect("provider import should preserve secret files");

    let imported_gemini = target_state
        .profiles
        .get("gemini-main")
        .expect("Gemini profile should exist");
    let imported_anthropic = target_state
        .profiles
        .get("claude-main")
        .expect("Anthropic profile should exist");
    assert_eq!(
        fs::read_to_string(imported_gemini.codex_home.join(GEMINI_OAUTH_SECRET_FILE))
            .expect("Gemini secret should import"),
        gemini_secret
    );
    assert_eq!(
        fs::read_to_string(imported_anthropic.codex_home.join(CLAUDE_CREDENTIALS_FILE))
            .expect("Claude secret should import"),
        claude_secret
    );
    assert!(!imported_gemini.codex_home.join("auth.json").exists());
    assert!(!imported_anthropic.codex_home.join("auth.json").exists());
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
            secret_files: Vec::new(),
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

#[test]
fn profile_export_non_tty_without_flags_requires_explicit_password_mode() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-export-non-tty-default");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let _password_guard = TestEnvVarGuard::unset("PRODEX_PROFILE_EXPORT_PASSWORD");
    seed_profile_export_state();
    let output_path = sandbox_dir.path.join("bundle.json");

    let err = handle_export_profiles(ExportProfileArgs {
        profile: Vec::new(),
        output: Some(output_path.clone()),
        password_protect: false,
        no_password: false,
    })
    .expect_err("non-interactive export without explicit mode should fail");

    assert!(
        err.to_string().contains(
            "non-interactive profile export requires --password-protect with PRODEX_PROFILE_EXPORT_PASSWORD set, or --no-password"
        ),
        "unexpected error: {err:#}"
    );
    assert!(!output_path.exists());
}

#[test]
fn profile_export_no_password_explicitly_writes_plain_bundle() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-export-no-password");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let _password_guard = TestEnvVarGuard::unset("PRODEX_PROFILE_EXPORT_PASSWORD");
    seed_profile_export_state();
    let output_path = sandbox_dir.path.join("bundle.json");

    handle_export_profiles(ExportProfileArgs {
        profile: Vec::new(),
        output: Some(output_path.clone()),
        password_protect: false,
        no_password: true,
    })
    .expect("explicit plaintext export should succeed");

    let envelope = read_profile_export_envelope_for_test(&output_path);
    assert!(matches!(&envelope, ProfileExportEnvelope::Plain { .. }));
    let decoded = prodex_profile_export::decode_profile_export_envelope(envelope, || {
        unreachable!("plain export should not request a password")
    })
    .expect("plain export should decode");
    assert_eq!(decoded.profiles.len(), 1);
    assert_eq!(decoded.profiles[0].name, "main");
}

#[test]
fn profile_export_password_protect_uses_env_password_in_non_tty() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-export-env-password");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let _password_guard = TestEnvVarGuard::set("PRODEX_PROFILE_EXPORT_PASSWORD", "secret-password");
    seed_profile_export_state();
    let output_path = sandbox_dir.path.join("bundle.json");

    handle_export_profiles(ExportProfileArgs {
        profile: Vec::new(),
        output: Some(output_path.clone()),
        password_protect: true,
        no_password: false,
    })
    .expect("password-protected export should succeed with env password");

    let envelope = read_profile_export_envelope_for_test(&output_path);
    assert!(matches!(
        &envelope,
        ProfileExportEnvelope::Encrypted { .. }
    ));
    let decoded = prodex_profile_export::decode_profile_export_envelope(envelope, || {
        Ok("secret-password".to_string())
    })
    .expect("encrypted export should decode with env password");
    assert_eq!(decoded.profiles.len(), 1);
    assert_eq!(decoded.profiles[0].name, "main");
}

#[cfg(unix)]
#[test]
fn profile_export_bundle_is_written_with_private_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let target_dir = ProfileCommandsTestDir::new("export-bundle-permissions");
    let output_path = target_dir.path.join("bundle.json");

    prodex_profile_export::write_profile_export_bundle(&output_path, b"{\"secret\":true}")
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
fn profile_import_kiro_reads_provider_metadata_from_local_cli_state() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let kiro_data_dir = sandbox_dir.path.join("home/.local/share/kiro-cli");
    fs::create_dir_all(&kiro_data_dir).expect("kiro data dir should be created");
    let database_path = kiro_data_dir.join("data.sqlite3");
    let connection = rusqlite::Connection::open(&database_path).expect("kiro db should open");
    connection
        .execute_batch(
            r#"
            CREATE TABLE auth_kv (key TEXT PRIMARY KEY, value TEXT);
            CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT);
            "#,
        )
        .expect("kiro schema should be created");
    connection
        .execute(
            "INSERT INTO auth_kv(key, value) VALUES(?1, ?2)",
            rusqlite::params![
                "codewhisperer:odic:token",
                serde_json::json!({
                    "access_token": "kiro-access-token",
                    "refresh_token": "kiro-refresh-token",
                    "start_url": "https://view.awsapps.com/start",
                    "region": "us-east-1"
                })
                .to_string()
            ],
        )
        .expect("kiro token should be stored");
    connection
        .execute(
            "INSERT INTO state(key, value) VALUES(?1, ?2)",
            rusqlite::params![
                "api.codewhisperer.profile",
                serde_json::json!({
                    "arn": "arn:aws:codewhisperer:us-east-1:123456789012:profile/test",
                    "profile_name": "builder-id-test",
                    "user_id": "kiro-user@example.com"
                })
                .to_string()
            ],
        )
        .expect("kiro profile state should be stored");
    connection
        .execute(
            "INSERT INTO state(key, value) VALUES(?1, ?2)",
            rusqlite::params!["auth.idc.region", "us-east-1"],
        )
        .expect("kiro region should be stored");

    handle_import_profiles(ImportProfileArgs {
        path: PathBuf::from("kiro"),
        name: Some("kiro-main".to_string()),
        activate: true,
    })
    .expect("kiro import should succeed");

    let paths = AppPaths::discover().expect("paths should resolve");
    let state = AppState::load(&paths).expect("state should load");
    let profile = state
        .profiles
        .get("kiro-main")
        .expect("kiro profile should exist");
    assert!(profile.managed);
    assert_eq!(state.active_profile.as_deref(), Some("kiro-main"));
    assert_eq!(profile.email.as_deref(), Some("kiro-user@example.com"));
    let imported_secret =
        read_kiro_auth_secret(&profile.codex_home).expect("kiro secret should be readable");
    assert_eq!(imported_secret.auth_key, "codewhisperer:odic:token");
    assert_eq!(imported_secret.auth_kind, "builder-id");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&imported_secret.auth_json)
            .expect("embedded auth json should parse")["access_token"]
            .as_str(),
        Some("kiro-access-token")
    );
    match &profile.provider {
        ProfileProvider::Kiro {
            auth_key,
            auth_kind,
            profile_arn,
            profile_name,
            start_url,
            region,
        } => {
            assert_eq!(auth_key, "codewhisperer:odic:token");
            assert_eq!(auth_kind.as_deref(), Some("builder-id"));
            assert_eq!(
                profile_arn.as_deref(),
                Some("arn:aws:codewhisperer:us-east-1:123456789012:profile/test")
            );
            assert_eq!(profile_name.as_deref(), Some("builder-id-test"));
            assert_eq!(start_url.as_deref(), Some("https://view.awsapps.com/start"));
            assert_eq!(region.as_deref(), Some("us-east-1"));
        }
        other => panic!("expected kiro provider, got {other:?}"),
    }
}

fn seed_profile_export_state() {
    let paths = AppPaths::discover().expect("paths should resolve");
    let main_home = paths.managed_profiles_root.join("main");
    profile_commands_write_profile_auth(&main_home, "main");
    AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: main_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        ..AppState::default()
    }
    .save(&paths)
    .expect("state should save");
}

fn read_profile_export_envelope_for_test(path: &Path) -> ProfileExportEnvelope {
    serde_json::from_slice(
        &fs::read(path).expect("profile export bundle should be readable"),
    )
    .expect("profile export bundle should parse")
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
    assert_eq!(
        info.access_type_sku.as_deref(),
        Some("free_limited_copilot")
    );
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
fn profile_export_round_trip_preserves_kiro_provider_metadata() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let source_dir = ProfileCommandsTestDir::new("kiro-export-source");
    let source_paths = profile_commands_test_paths(&source_dir.path);
    let kiro_home = source_paths.managed_profiles_root.join("kiro-main");
    create_codex_home_if_missing(&kiro_home).expect("kiro home should exist");
    let kiro_secret = serde_json::json!({
        "auth_key": "codewhisperer:odic:token",
        "auth_kind": "builder-id",
        "auth_json": serde_json::json!({
            "access_token": "kiro-access-token",
            "refresh_token": "kiro-refresh-token",
            "start_url": "https://view.awsapps.com/start",
            "region": "us-east-1"
        })
        .to_string(),
        "email": "kiro-user@example.com",
        "profile_arn": "arn:aws:codewhisperer:us-east-1:123456789012:profile/test",
        "profile_name": "builder-id-test",
        "start_url": "https://view.awsapps.com/start",
        "region": "us-east-1"
    })
    .to_string();
    write_secret_text_file(&kiro_home.join(KIRO_CREDENTIALS_FILE), &kiro_secret)
        .expect("kiro secret should be written");
    write_secret_text_file(
        &kiro_home.join(KIRO_MODEL_CATALOG_FILE),
        &serde_json::json!({
            "models": [
                { "id": "claude-sonnet-4", "name": "claude-sonnet-4", "object": "model", "owned_by": "kiro-cli" },
                { "id": "claude-sonnet-4.5", "name": "claude-sonnet-4.5", "object": "model", "owned_by": "kiro-cli" }
            ]
        })
        .to_string(),
    )
    .expect("kiro model catalog should be written");

    let source_state = AppState {
        active_profile: Some("kiro-main".to_string()),
        profiles: BTreeMap::from([(
            "kiro-main".to_string(),
            ProfileEntry {
                codex_home: kiro_home,
                managed: true,
                email: Some("kiro-user@example.com".to_string()),
                provider: ProfileProvider::Kiro {
                    auth_key: "codewhisperer:odic:token".to_string(),
                    auth_kind: Some("builder-id".to_string()),
                    profile_arn: Some(
                        "arn:aws:codewhisperer:us-east-1:123456789012:profile/test".to_string(),
                    ),
                    profile_name: Some("builder-id-test".to_string()),
                    start_url: Some("https://view.awsapps.com/start".to_string()),
                    region: Some("us-east-1".to_string()),
                },
            },
        )]),
        ..AppState::default()
    };

    let payload = build_profile_export_payload(&source_state, &["kiro-main".to_string()])
        .expect("payload should build");
    assert_eq!(payload.profiles.len(), 1);
    assert!(payload.profiles[0].auth_json.is_empty());
    assert_eq!(payload.profiles[0].secret_files.len(), 2);
    assert_eq!(payload.profiles[0].secret_files[0].path, KIRO_CREDENTIALS_FILE);
    assert_eq!(
        parse_kiro_auth_secret_text(&payload.profiles[0].secret_files[0].text)
            .expect("exported Kiro secret should parse")
            .auth_key,
        "codewhisperer:odic:token"
    );
    assert_eq!(payload.profiles[0].secret_files[1].path, KIRO_MODEL_CATALOG_FILE);
    assert_eq!(
        parse_kiro_model_catalog_text(&payload.profiles[0].secret_files[1].text)
            .expect("exported Kiro model catalog should parse")
            .len(),
        2
    );

    let target_dir = ProfileCommandsTestDir::new("kiro-export-target");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let mut target_state = AppState::default();
    import_profile_export_payload(&target_paths, &mut target_state, &payload)
        .expect("kiro import should succeed");

    let imported = target_state
        .profiles
        .get("kiro-main")
        .expect("kiro profile should be imported");
    assert_eq!(imported.email.as_deref(), Some("kiro-user@example.com"));
    assert!(!imported.codex_home.join("auth.json").exists());
    assert_eq!(
        fs::read_to_string(imported.codex_home.join(KIRO_CREDENTIALS_FILE))
            .expect("kiro secret should import"),
        kiro_secret
    );
    assert_eq!(
        parse_kiro_model_catalog_text(
            &fs::read_to_string(imported.codex_home.join(KIRO_MODEL_CATALOG_FILE))
                .expect("kiro model catalog should import")
        )
        .expect("imported Kiro model catalog should parse")
        .len(),
        2
    );
    match &imported.provider {
        ProfileProvider::Kiro {
            auth_key,
            auth_kind,
            profile_arn,
            profile_name,
            start_url,
            region,
        } => {
            assert_eq!(auth_key, "codewhisperer:odic:token");
            assert_eq!(auth_kind.as_deref(), Some("builder-id"));
            assert_eq!(
                profile_arn.as_deref(),
                Some("arn:aws:codewhisperer:us-east-1:123456789012:profile/test")
            );
            assert_eq!(profile_name.as_deref(), Some("builder-id-test"));
            assert_eq!(start_url.as_deref(), Some("https://view.awsapps.com/start"));
            assert_eq!(region.as_deref(), Some("us-east-1"));
        }
        other => panic!("expected kiro provider, got {other:?}"),
    }
}
