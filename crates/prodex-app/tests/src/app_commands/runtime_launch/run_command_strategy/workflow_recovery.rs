use super::*;
use std::io::Write;

fn workflow_fixture() -> (PathBuf, TestEnvVarGuard, TestEnvVarGuard, PathBuf, String) {
    let root = temp_dir("workflow-recovery");
    let home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_home = root.join("shared-codex-home");
    let shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_home.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let session_id = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9".to_string();
    let sessions = paths.shared_codex_root.join("sessions/2026/09/06");
    fs::create_dir_all(&sessions).unwrap();
    let session_path = sessions.join(format!("rollout-2026-09-06T01-00-00-{session_id}.jsonl"));
    fs::write(
        &session_path,
        session_meta_line(&session_id, &root, Some("openai")),
    )
    .unwrap();
    writeln!(
        fs::OpenOptions::new()
            .append(true)
            .open(&session_path)
            .unwrap(),
        "{}",
        serde_json::json!({
            "type": "turn_context",
            "payload": {"model": "gpt-5.6-luna", "effort": "max"}
        })
    )
    .unwrap();

    let mut profiles = BTreeMap::new();
    for name in ["main", "second"] {
        let profile_home = root.join("profiles").join(name);
        fs::create_dir_all(&profile_home).unwrap();
        write_runtime_launch_auth(
            secret_store::auth_json_path(&profile_home),
            format!(r#"{{"tokens":{{"access_token":"{name}-token"}}}}"#),
        )
        .unwrap();
        profiles.insert(
            name.to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: false,
                email: None,
                provider: ProfileProvider::Openai,
            },
        );
    }
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles,
            session_profile_bindings: BTreeMap::from([(
                session_id.clone(),
                ResponseProfileBinding {
                    binding_identity: None,
                    profile_name: "main".to_string(),
                    bound_at: chrono::Local::now().timestamp(),
                },
            )]),
            ..AppState::default()
        },
    );
    (root, home, shared, session_path, session_id)
}

#[test]
fn structured_terminal_error_relaunches_the_same_session_without_replay() {
    let (_root, _home, _shared, session_path, session_id) = workflow_fixture();
    let mut strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: true,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from(&session_id),
            OsString::from("original prompt must not be replayed"),
        ],
    })
    .unwrap();
    let mut rollout = fs::OpenOptions::new()
        .append(true)
        .open(&session_path)
        .unwrap();
    writeln!(
        rollout,
        "{}",
        serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "original prompt must not be replayed"}]
            }
        })
    )
    .unwrap();
    writeln!(
        rollout,
        "{}",
        serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "model_reroute",
                "from_model": "gpt-5.6-luna",
                "to_model": "gpt-5.2",
                "reason": "high_risk_cyber_activity"
            }
        })
    )
    .unwrap();
    writeln!(
        rollout,
        "{}",
        serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "agent_message", "message": "committed output"}
        })
    )
    .unwrap();
    writeln!(
        rollout,
        "{}",
        serde_json::json!({
            "type": "event_msg",
            "payload": {"type": "tool_completed", "call_id": "effect-once"}
        })
    )
    .unwrap();
    writeln!(
        rollout,
        "{}",
        serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "error",
                "codex_error_info": {"response_too_many_failed_attempts": {"http_status_code": 503}}
            }
        })
    )
    .unwrap();
    drop(rollout);
    let before_relaunch = fs::read(&session_path).unwrap();

    assert!(strategy.child_exit_requested().unwrap());
    assert_eq!(strategy.recovery_model.as_deref(), Some("gpt-5.2"));
    let pending = strategy.pending_goal_resume_plan.clone();
    assert_eq!(
        pending.as_ref().unwrap().evidence.acceptance_state,
        "side_effect_observed"
    );
    assert_eq!(
        pending.as_ref().unwrap().evidence.stream_committed,
        Some(true)
    );
    assert_eq!(
        pending.as_ref().unwrap().evidence.side_effect_state,
        "observed"
    );
    assert!(strategy.child_exit_requested().unwrap());
    assert_eq!(strategy.pending_goal_resume_plan, pending);
    assert!(strategy.relaunch_after_child_exit(&exit_status(1)).unwrap());
    assert!(!strategy.relaunch_after_child_exit(&exit_status(0)).unwrap());
    assert_eq!(strategy.args.profile.as_deref(), Some("second"));
    assert_eq!(strategy.recovery_generation, 1);
    assert_eq!(
        strategy.auto_goal_resume_attempted_profiles,
        BTreeSet::from(["main".to_string(), "second".to_string()])
    );
    assert_eq!(strategy.resume_session_id(), Some(session_id.as_str()));
    assert!(
        !strategy
            .codex_args
            .iter()
            .any(|arg| arg == "original prompt must not be replayed")
    );
    assert_eq!(
        strategy.codex_args.last().and_then(|arg| arg.to_str()),
        Some(RUNTIME_SESSION_CONTINUATION_PROMPT)
    );
    assert_eq!(fs::read(&session_path).unwrap(), before_relaunch);
    assert_eq!(
        String::from_utf8_lossy(&before_relaunch)
            .matches("original prompt must not be replayed")
            .count(),
        1
    );
    assert_eq!(
        String::from_utf8_lossy(&before_relaunch)
            .matches("effect-once")
            .count(),
        1
    );
}

#[test]
fn transient_pool_waits_then_reconsiders_a_recovered_profile() {
    let (_root, _home, _shared, session_path, session_id) = workflow_fixture();
    let mut strategy = RunCommandStrategy::new(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: true,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from("resume"), OsString::from(&session_id)],
    })
    .unwrap();
    let append_failure = || {
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(&session_path)
                .unwrap(),
            "{}",
            serde_json::json!({
                "type": "response_item",
                "payload": {"type": "message", "role": "user", "content": []}
            })
        )
        .unwrap();
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(&session_path)
                .unwrap(),
            "{}",
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "type": "error",
                    "codex_error_info": {
                        "response_too_many_failed_attempts": {"http_status_code": 503}
                    }
                }
            })
        )
        .unwrap();
    };

    append_failure();
    assert!(strategy.child_exit_requested().unwrap());
    assert!(strategy.relaunch_after_child_exit(&exit_status(1)).unwrap());
    assert_eq!(strategy.args.profile.as_deref(), Some("second"));
    assert_eq!(strategy.recovery_generation, 1);

    append_failure();
    let wait_started = std::time::Instant::now();
    assert!(strategy.relaunch_after_child_exit(&exit_status(1)).unwrap());
    assert!(wait_started.elapsed() >= std::time::Duration::from_secs(5));
    assert_eq!(strategy.transient_recovery_rounds, 1);
    assert_eq!(strategy.args.profile.as_deref(), Some("main"));
    assert_eq!(strategy.recovery_generation, 2);
}

#[test]
fn cancellation_and_policy_errors_do_not_start_session_recovery() {
    for (label, record) in [
        (
            "cancelled",
            serde_json::json!({
                "method": "turn/completed",
                "params": {
                    "turn": {
                        "status": "cancelled",
                        "error": {"codexErrorInfo": "usageLimitExceeded"}
                    }
                }
            }),
        ),
        (
            "policy",
            serde_json::json!({
                "type": "event_msg",
                "payload": {"type": "error", "codex_error_info": "cyber_policy"}
            }),
        ),
        (
            "acceptance_ambiguous",
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "type": "error",
                    "codex_error_info": {
                        "response_too_many_failed_attempts": {"http_status_code": 503}
                    }
                }
            }),
        ),
    ] {
        let (_root, _home, _shared, session_path, session_id) = workflow_fixture();
        let mut strategy = RunCommandStrategy::new(RunArgs {
            profile: None,
            auto_rotate: false,
            no_auto_rotate: false,
            auto_redeem: false,
            skip_quota_check: true,
            full_access: false,
            base_url: None,
            no_proxy: true,
            dry_run: false,
            codex_features: CodexRuntimeFeatureArgs::default(),
            codex_args: vec![OsString::from("resume"), OsString::from(session_id)],
        })
        .unwrap();
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(session_path)
                .unwrap(),
            "{record}"
        )
        .unwrap();
        assert!(!strategy.child_exit_requested().unwrap(), "{label}");
    }
}

#[test]
fn hard_affinity_entitlement_errors_recover_to_the_next_profile() {
    for (label, message, expected_class) in [
        (
            "upgrade",
            "unexpected status 403 Forbidden: You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro)",
            "usage_limit",
        ),
        (
            "deactivated",
            "unexpected status 403 Forbidden: {\"detail\":{\"code\":\"deactivated_workspace\",\"message\":\"workspace unavailable\"}}",
            "profile_unavailable",
        ),
    ] {
        let (_root, _home, _shared, session_path, session_id) = workflow_fixture();
        let mut strategy = RunCommandStrategy::new(RunArgs {
            profile: Some("main".to_string()),
            auto_rotate: false,
            no_auto_rotate: false,
            auto_redeem: false,
            skip_quota_check: true,
            full_access: false,
            base_url: None,
            no_proxy: true,
            dry_run: false,
            codex_features: CodexRuntimeFeatureArgs::default(),
            codex_args: vec![OsString::from("resume"), OsString::from(&session_id)],
        })
        .unwrap();
        let mut rollout = fs::OpenOptions::new()
            .append(true)
            .open(session_path)
            .unwrap();
        writeln!(
            rollout,
            "{}",
            serde_json::json!({
                "type": "event_msg",
                "payload": {"type": "turn_started", "turn_id": "turn-2"}
            })
        )
        .unwrap();
        writeln!(
            rollout,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "payload": {"type": "message", "role": "user", "content": []}
            })
        )
        .unwrap();
        writeln!(
            rollout,
            "{}",
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "type": "error",
                    "message": message,
                    "codex_error_info": "other"
                }
            })
        )
        .unwrap();
        drop(rollout);

        assert!(strategy.child_exit_requested().unwrap(), "{label}");
        let plan = strategy.pending_goal_resume_plan.as_ref().unwrap();
        assert_eq!(plan.profile_name, "second", "{label}");
        assert_eq!(plan.failure_class, expected_class, "{label}");
        assert_eq!(plan.evidence.acceptance_state, "accepted_but_uncommitted");
    }
}

#[test]
fn cancellation_discards_a_pending_live_recovery_plan() {
    let (_root, _home, _shared, _session_path, session_id) = workflow_fixture();
    let mut strategy = RunCommandStrategy::new(RunArgs {
        profile: Some("main".to_string()),
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: true,
        full_access: false,
        base_url: None,
        no_proxy: true,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from("resume"), OsString::from(&session_id)],
    })
    .unwrap();
    strategy.pending_goal_resume_plan = Some(GoalResumeRelaunchPlan {
        session_id,
        failed_profile_name: "main".to_string(),
        profile_name: "second".to_string(),
        failure_class: "transport",
        resume_goal: false,
        evidence: Default::default(),
    });

    assert!(
        !strategy
            .relaunch_after_child_exit(&exit_status(130))
            .unwrap()
    );
    assert!(strategy.pending_goal_resume_plan.is_none());
    assert_eq!(strategy.args.profile.as_deref(), Some("main"));
}
