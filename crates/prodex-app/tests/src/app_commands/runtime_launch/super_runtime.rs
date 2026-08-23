use super::secure_fixture::session_meta_line;
use super::{
    AppState, BTreeMap, ProfileEntry, ProfileProvider, RUNTIME_PROXY_OPENAI_MOUNT_PATH,
    RuntimeLaunchRequest, SuperExternalProvider, TestEnvVarGuard, fs, prepare_runtime_launch,
    prepare_runtime_launch_dry_run, resolved_super_runtime_tool_args, temp_dir,
    write_runtime_launch_auth, write_state,
};
use crate::app_commands::runtime_launch::{
    repair_super_resume_session_metadata, resolve_super_dry_run_main_agent,
};
use crate::{AppPaths, codex_cli_config_override_value};

#[test]
fn super_picker_and_direct_resume_repair_the_same_stale_overlay_path() {
    let root = temp_dir("super-resume-stale-overlay");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "01a01824-29f7-7332-96c7-5d09044ee2d0";
    let relative = format!("sessions/2026/08/19/rollout-2026-08-19T10-50-18-{session_id}.jsonl");
    let rollout_path = paths.shared_codex_root.join(&relative);
    fs::create_dir_all(rollout_path.parent().unwrap()).unwrap();
    fs::write(
        &rollout_path,
        format!(
            "{{\"timestamp\":\"2026-08-19T10:50:18Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"session_id\":\"{session_id}\",\"timestamp\":\"2026-08-19T10:50:18Z\",\"cwd\":\"/home/test-user/project\",\"originator\":\"codex-cli\",\"cli_version\":\"0.148.0\",\"model_provider\":\"openai\"}}}}\n"
        ),
    )
    .unwrap();
    let stale_path = paths
        .shared_codex_root
        .with_file_name(".prodex-overlay-old")
        .join(&relative);
    let connection =
        rusqlite::Connection::open(paths.shared_codex_root.join("state_5.sqlite")).unwrap();
    connection
        .execute(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO threads (id, rollout_path) VALUES (?1, ?2)",
            rusqlite::params![session_id, stale_path.display().to_string()],
        )
        .unwrap();

    let repaired_path = |connection: &rusqlite::Connection| -> String {
        connection
            .query_row(
                "SELECT rollout_path FROM threads WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .unwrap()
    };
    let crate::Commands::Super(picker_args) =
        crate::parse_cli_command_from(["prodex", "s"]).unwrap()
    else {
        panic!("expected Super command");
    };
    repair_super_resume_session_metadata(&picker_args).unwrap();
    assert_eq!(
        std::path::PathBuf::from(repaired_path(&connection)),
        rollout_path
    );

    connection
        .execute(
            "UPDATE threads SET rollout_path = ?1 WHERE id = ?2",
            rusqlite::params![stale_path.display().to_string(), session_id],
        )
        .unwrap();
    let crate::Commands::Super(direct_args) =
        crate::parse_cli_command_from(["prodex", "s", session_id]).unwrap()
    else {
        panic!("expected Super command");
    };
    repair_super_resume_session_metadata(&direct_args).unwrap();
    assert_eq!(
        std::path::PathBuf::from(repaired_path(&connection)),
        rollout_path
    );
}

#[test]
fn super_resume_restores_the_session_model_and_reasoning_effort() {
    let root = temp_dir("super-resume-session-settings");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44fa";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        format!(
            "{}{}",
            session_meta_line(session_id, &root, Some("openai")),
            r#"{"timestamp":"2026-06-05T01:01:00Z","type":"turn_context","payload":{"model":"gpt-5.6-luna","effort":"max"}}
{"timestamp":"2026-06-05T01:02:00Z","type":"response_item","payload":{"id":"rs_response_item_id"}}
"#
        ),
    )
    .unwrap();
    let command =
        crate::parse_cli_command_from(["prodex", "s", "--no-sub-agent", session_id]).unwrap();
    let crate::Commands::Super(args) = command else {
        panic!("expected Super command");
    };

    let runtime_args = resolved_super_runtime_tool_args(args, false);

    assert_eq!(
        codex_cli_config_override_value(&runtime_args.codex_args, "model").as_deref(),
        Some("gpt-5.6-luna")
    );
    assert_eq!(
        codex_cli_config_override_value(&runtime_args.codex_args, "model_reasoning_effort")
            .as_deref(),
        Some("max")
    );
}

#[test]
fn super_resume_dry_run_restores_session_provider_model_and_reasoning_effort() {
    let root = temp_dir("super-resume-dry-run-session-settings");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_codex_home = root.join("shared-codex-home");
    let _shared_env = TestEnvVarGuard::set(
        "PRODEX_SHARED_CODEX_HOME",
        shared_codex_home.to_str().unwrap(),
    );
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44fb";
    let sessions = paths.shared_codex_root.join("sessions/2026/06/05");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(
        sessions.join("rollout.jsonl"),
        format!(
            "{}{}",
            session_meta_line(session_id, &root, Some("prodex-kiro")),
            r#"{"timestamp":"2026-06-05T01:01:00Z","type":"turn_context","payload":{"model":"gpt-5.6-luna","effort":"max"}}
"#
        ),
    )
    .unwrap();
    let command =
        crate::parse_cli_command_from(["prodex", "s", "--dry-run", "--no-sub-agent", session_id])
            .unwrap();
    let crate::Commands::Super(mut args) = command else {
        panic!("expected Super command");
    };
    args.extract_provider_overrides_from_codex_args().unwrap();

    resolve_super_dry_run_main_agent(&mut args).unwrap();
    let runtime_args = resolved_super_runtime_tool_args(args, false);

    assert_eq!(
        runtime_args.external_provider,
        Some(SuperExternalProvider::Kiro)
    );
    assert_eq!(
        codex_cli_config_override_value(&runtime_args.codex_args, "model").as_deref(),
        Some("gpt-5.6-luna")
    );
    assert_eq!(
        codex_cli_config_override_value(&runtime_args.codex_args, "model_reasoning_effort")
            .as_deref(),
        Some("max")
    );
}

#[test]
fn prepare_runtime_launch_enables_runtime_proxy_for_openai_smart_context_single_profile() {
    let root = temp_dir("smart-context-single-profile-runtime-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_runtime_launch_auth(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(65_536),
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();

    assert_eq!(
        prepared
            .runtime_proxy
            .as_ref()
            .expect("Smart Context should force runtime proxy for OpenAI")
            .openai_mount_path,
        RUNTIME_PROXY_OPENAI_MOUNT_PATH
    );
}

#[test]
fn prepare_runtime_launch_dry_run_previews_proxy_for_presidio_redaction() {
    let root = temp_dir("presidio-dry-run-runtime-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_runtime_launch_auth(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );

    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: true,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();

    assert_eq!(
        prepared
            .runtime_proxy
            .as_ref()
            .expect("Presidio should force runtime proxy preview")
            .openai_mount_path,
        RUNTIME_PROXY_OPENAI_MOUNT_PATH
    );
}
