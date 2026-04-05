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

struct ProfileCommandsEnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl ProfileCommandsEnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = env::var_os(key);
        unsafe { env::set_var(key, value) };
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = env::var_os(key);
        unsafe { env::remove_var(key) };
        Self { key, previous }
    }
}

impl Drop for ProfileCommandsEnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = self.previous.as_ref() {
            unsafe { env::set_var(self.key, value) };
        } else {
            unsafe { env::remove_var(self.key) };
        }
    }
}

struct ProfileCommandsTestEnv {
    _home_guard: ProfileCommandsEnvVarGuard,
    _prodex_guard: ProfileCommandsEnvVarGuard,
    _shared_override_guard: ProfileCommandsEnvVarGuard,
}

impl ProfileCommandsTestEnv {
    fn new(root: &Path) -> Self {
        let home = root.join("home");
        let prodex_home = root.join("prodex");
        fs::create_dir_all(&home).expect("test home should be created");
        fs::create_dir_all(&prodex_home).expect("test prodex home should be created");
        Self {
            _home_guard: ProfileCommandsEnvVarGuard::set("HOME", &home.display().to_string()),
            _prodex_guard: ProfileCommandsEnvVarGuard::set(
                "PRODEX_HOME",
                &prodex_home.display().to_string(),
            ),
            _shared_override_guard: ProfileCommandsEnvVarGuard::unset("PRODEX_SHARED_CODEX_HOME"),
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

fn profile_commands_write_profile_auth(codex_home: &Path, profile_name: &str) {
    create_codex_home_if_missing(codex_home).expect("profile home should exist");
    write_secret_text_file(
        &codex_home.join("auth.json"),
        &profile_commands_sample_auth_json(profile_name),
    )
    .expect("auth.json should be written");
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
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: false,
                    email: Some("second@example.com".to_string()),
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

#[test]
fn profile_import_rejects_existing_profile_names() {
    let sandbox_dir = ProfileCommandsTestDir::new("profile-commands-env");
    let _env = ProfileCommandsTestEnv::new(&sandbox_dir.path);
    let target_dir = ProfileCommandsTestDir::new("import-collision");
    let target_paths = profile_commands_test_paths(&target_dir.path);
    let existing_home = target_paths.managed_profiles_root.join("main");
    profile_commands_write_profile_auth(&existing_home, "main");

    let existing_state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: existing_home,
                managed: true,
                email: Some("main@example.com".to_string()),
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
            auth_json: profile_commands_sample_auth_json("main"),
        }],
    };

    let err = stage_imported_profiles(&target_paths, &existing_state, &payload)
        .expect_err("import should reject duplicate profile names");
    assert!(err.to_string().contains("already exists"));
}
