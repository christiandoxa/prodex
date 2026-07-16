use super::*;

#[test]
fn main_entry_exit_code_respects_command_exit_errors() {
    let err = command_dispatch::command_exit_error(42, "command-specific failure");

    assert_eq!(main_entry_exit_code(&err), 42);
    assert_eq!(format!("{err:#}"), "command-specific failure");
}

#[test]
fn main_entry_exit_code_defaults_to_one_for_generic_errors() {
    let err = anyhow::anyhow!("generic failure");

    assert_eq!(main_entry_exit_code(&err), 1);
}

#[test]
fn main_entry_treats_nested_broken_pipe_as_success() {
    let error = anyhow::Error::new(io::Error::new(io::ErrorKind::BrokenPipe, "closed pipe"))
        .context("failed to print command output");

    assert!(main_entry_error_is_broken_pipe(&error));
}

#[test]
fn main_entry_does_not_hide_other_io_errors() {
    let error = anyhow::Error::new(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));

    assert!(!main_entry_error_is_broken_pipe(&error));
}

#[test]
fn invalid_runtime_policy_does_not_block_doctor_or_profile_recovery() {
    let root = std::env::temp_dir().join(format!(
        "prodex-command-preflight-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("policy.toml"), "version = [invalid").unwrap();
    let _home = TestEnvVarGuard::set("PRODEX_HOME", &root.to_string_lossy());
    let shared_codex_home = root.join("shared-codex");
    let _shared_home = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        &shared_codex_home.to_string_lossy(),
    );
    clear_runtime_policy_cache();

    let doctor =
        parse_cli_command_from(["prodex", "doctor", "--runtime", "--json"]).unwrap();
    let profiles = parse_cli_command_from(["prodex", "profile", "list"]).unwrap();
    let runtime = parse_cli_command_from(["prodex", "run"]).unwrap();

    assert!(validate_command_runtime_policy(&doctor).is_ok());
    assert!(validate_command_runtime_policy(&profiles).is_ok());
    assert!(validate_command_runtime_policy(&runtime).is_err());
    doctor.execute().expect("doctor should diagnose invalid policy");
    profiles
        .execute()
        .expect("profile recovery should ignore invalid policy");

    clear_runtime_policy_cache();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn main_entry_error_message_redacts_secret_like_chain() {
    let err = anyhow::anyhow!("failed: Authorization: Bearer main-entry-token")
        .context("command failed");

    let message = main_entry_error_message(&err);

    assert!(message.contains("command failed"));
    assert!(message.contains("Authorization: Bearer <redacted>"));
    assert!(!message.contains("main-entry-token"));
}

#[test]
fn startup_probe_refresh_targets_current_then_stale_or_missing_profiles() {
    let temp_dir = TestDir::new();
    let now = Local::now().timestamp();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    let fourth_home = temp_dir.path.join("homes/fourth");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");
    write_auth_json(&fourth_home.join("auth.json"), "fourth-account");

    let state = AppState {
        active_profile: Some("third".to_string()),
        profiles: BTreeMap::from([
            (
                "fourth".to_string(),
                ProfileEntry {
                    codex_home: fourth_home,
                    managed: true,
                    email: Some("fourth@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
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
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    let probe_cache = BTreeMap::from([(
        "fourth".to_string(),
        RuntimeProfileProbeCacheEntry {
            checked_at: now,
            auth: AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            result: Ok(usage_with_main_windows(90, 18_000, 90, 604_800)),
        },
    )]);
    let usage_snapshots = BTreeMap::from([(
        "second".to_string(),
        RuntimeProfileUsageSnapshot {
            checked_at: now,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 85,
            five_hour_reset_at: now + 18_000,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 92,
            weekly_reset_at: now + 604_800,
        },
    )]);

    assert_eq!(
        runtime_profiles_needing_startup_probe_refresh(
            &state,
            "third",
            &probe_cache,
            &usage_snapshots,
            now,
        ),
        vec!["third".to_string(), "main".to_string()]
    );
}

#[test]
fn startup_probe_refresh_warms_current_profiles_when_snapshots_are_empty() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    let third_home = temp_dir.path.join("homes/third");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");
    write_auth_json(&third_home.join("auth.json"), "third-account");

    let state = AppState {
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
                    codex_home: second_home,
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "third".to_string(),
                ProfileEntry {
                    codex_home: third_home,
                    managed: true,
                    email: Some("third@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };

    assert_eq!(
        runtime_profiles_needing_startup_probe_refresh(
            &state,
            "second",
            &BTreeMap::new(),
            &BTreeMap::new(),
            Local::now().timestamp(),
        ),
        vec![
            "second".to_string(),
            "third".to_string(),
            "main".to_string()
        ]
    );
}
