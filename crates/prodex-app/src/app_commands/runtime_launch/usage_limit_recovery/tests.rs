use super::*;
use crate::app_state::AppStateIoExt;
use crate::test_support::TestEnvVarGuard;
use crate::{ProfileEntry, ProfileProvider, ResponseProfileBinding};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SESSION_ID: &str = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";

fn exit_status(code: i32) -> std::process::ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code as u32)
    }
}

fn fixture_root(label: &str) -> PathBuf {
    let root = crate::test_support::test_temp_root().join(format!(
        "prodex-usage-limit-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    secret_store::ensure_private_directory(&root).unwrap();
    root
}

fn configured_env(root: &Path) -> (TestEnvVarGuard, TestEnvVarGuard) {
    let home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared = root.join("shared-codex");
    let shared_guard = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared.to_str().unwrap());
    (home, shared_guard)
}

fn populate_fixture(root: &Path, paths: &AppPaths, profile_names: &[&str]) -> PathBuf {
    let mut profiles = BTreeMap::new();
    let profiles_root = root.join("profiles");
    fs::create_dir_all(&profiles_root).unwrap();
    secret_store::ensure_private_directory(&profiles_root).unwrap();
    for name in profile_names {
        let home = profiles_root.join(name);
        fs::create_dir_all(&home).unwrap();
        secret_store::ensure_private_directory(&home).unwrap();
        secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
            .write_text(
                &secret_store::SecretLocation::file(secret_store::auth_json_path(&home)),
                r#"{"tokens":{"access_token":"synthetic-token"}}"#,
            )
            .unwrap();
        profiles.insert(
            (*name).to_string(),
            ProfileEntry {
                codex_home: home,
                managed: false,
                email: None,
                provider: ProfileProvider::Openai,
            },
        );
    }
    let state = AppState {
        active_profile: profile_names.first().map(|name| (*name).to_string()),
        profiles,
        session_profile_bindings: BTreeMap::from([(
            SESSION_ID.to_string(),
            ResponseProfileBinding {
                binding_identity: None,
                profile_name: profile_names[0].to_string(),
                bound_at: chrono::Local::now().timestamp(),
            },
        )]),
        ..AppState::default()
    };
    state.save(paths).unwrap();

    let sessions = paths.shared_codex_root.join("sessions/2026/08/29");
    fs::create_dir_all(&sessions).unwrap();
    let session_path = sessions.join(format!("rollout-2026-08-29T01-00-00-{SESSION_ID}.jsonl"));
    fs::write(
        &session_path,
        format!(
            "{{\"timestamp\":\"2026-08-29T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{SESSION_ID}\",\"cwd\":\"/tmp/prodex-p0\",\"originator\":\"codex_exec\",\"cli_version\":\"0.151.0\",\"model_provider\":\"openai\"}}}}\n"
        ),
    )
    .unwrap();
    session_path
}

fn append_usage_limit(path: &Path) {
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    writeln!(
        file,
        "{{\"timestamp\":\"2026-08-29T01:00:01Z\",\"type\":\"event_msg\",\"payload\":{{\"message\":{}}}}}",
        serde_json::to_string(OBSERVED_USAGE_LIMIT_MESSAGE).unwrap()
    )
    .unwrap();
}

fn append_user_event(path: &Path) {
    writeln!(
        fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{}",
        serde_json::json!({
            "type": "response_item",
            "payload": {"type": "message", "role": "user", "content": []}
        })
    )
    .unwrap();
}

fn append_compressed_usage_limit(path: &Path) {
    let mut contents = zstd::stream::decode_all(fs::File::open(path).unwrap()).unwrap();
    contents.extend_from_slice(
        format!(
            "{{\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":[]}}}}\n{{\"timestamp\":\"2026-08-29T01:00:01Z\",\"type\":\"event_msg\",\"payload\":{{\"message\":{}}}}}\n",
            serde_json::to_string(OBSERVED_USAGE_LIMIT_MESSAGE).unwrap()
        )
        .as_bytes(),
    );
    fs::write(
        path,
        zstd::stream::encode_all(contents.as_slice(), 3).unwrap(),
    )
    .unwrap();
}

#[test]
fn ambiguous_legacy_usage_limit_does_not_replay_a_non_goal_session() {
    let root = fixture_root("legacy-ambiguous");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = prepare_runtime_usage_limit_monitor(
        &[OsString::from("exec"), OsString::from("work")],
        false,
    )
    .unwrap()
    .unwrap();
    super::super::goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
    )
    .unwrap();
    writeln!(
        fs::OpenOptions::new()
            .append(true)
            .open(session_path)
            .unwrap(),
        "{{\"type\":\"event_msg\",\"payload\":{{\"message\":{}}}}}",
        serde_json::to_string(OBSERVED_USAGE_LIMIT_MESSAGE).unwrap()
    )
    .unwrap();
    let attempted = BTreeSet::new();
    assert!(
        plan_runtime_usage_limit_relaunch(
            &mut monitor,
            &exit_status(1),
            &resume_options(&attempted, false, Some("a")),
        )
        .unwrap()
        .is_none()
    );
    let _ = fs::remove_dir_all(root);
}

fn resume_options<'a>(
    attempted_profiles: &'a BTreeSet<String>,
    no_auto_rotate: bool,
    requested_profile: Option<&'a str>,
) -> RuntimeUsageLimitResumeOptions<'a> {
    RuntimeUsageLimitResumeOptions {
        requested_profile,
        no_auto_rotate,
        skip_quota_check: true,
        base_url: None,
        include_code_review: false,
        no_proxy: false,
        requested_model: None,
        failure_class: "usage_limit",
        allow_failed_profile: false,
        resume_goal: false,
        attempted_profiles,
    }
}

#[test]
fn usage_limit_detection_accepts_error_envelopes_not_conversation_prose() {
    assert!(goal_resume_line_has_usage_limit(
        OBSERVED_USAGE_LIMIT_MESSAGE
    ));
    assert!(goal_resume_line_has_usage_limit(&format!(
        r#"{{"type":"event_msg","payload":{{"message":{}}}}}"#,
        serde_json::to_string(OBSERVED_USAGE_LIMIT_MESSAGE).unwrap()
    )));

    for value in [
        serde_json::json!({
            "type": "error",
            "payload": { "message": "You've hit your usage limit. Try again later." }
        }),
        serde_json::json!({
            "type": "response.failed",
            "response": { "error": { "code": "usage_limit_reached" } }
        }),
        serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "error",
                "error": { "type": "usage_not_included" }
            }
        }),
        serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "error",
                "message": "Quota unavailable",
                "codex_error_info": "usage_limit_exceeded"
            }
        }),
        serde_json::json!({ "error": { "code": "insufficient_quota" } }),
    ] {
        assert!(goal_resume_line_has_usage_limit(
            &serde_json::to_string(&value).unwrap()
        ));
    }

    for value in [
        serde_json::json!({
            "type": "message",
            "payload": {
                "role": "user",
                "content": OBSERVED_USAGE_LIMIT_MESSAGE
            }
        }),
        serde_json::json!({
            "type": "message",
            "payload": {
                "role": "assistant",
                "content": "The docs mention usage_limit_reached as an example."
            }
        }),
        serde_json::json!({
            "type": "event_msg",
            "payload": { "message": "The docs mention usage_limit_reached as an example." }
        }),
        serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "warning",
                "message": OBSERVED_USAGE_LIMIT_MESSAGE
            }
        }),
        serde_json::json!({
            "type": "error",
            "payload": {
                "type": "error",
                "message": "The docs mention usage_limit_reached as an example."
            }
        }),
    ] {
        assert!(!goal_resume_line_has_usage_limit(
            &serde_json::to_string(&value).unwrap()
        ));
    }
}

#[test]
fn post_exit_recovery_requires_a_matching_resumable_goal_when_goal_store_exists() {
    for (label, thread_id, status, expected_profile) in [
        ("paused", SESSION_ID, "paused", Some("b")),
        ("complete", SESSION_ID, "complete", None),
        ("unrelated", "other-thread", "paused", None),
    ] {
        let root = fixture_root(label);
        let (_home, _shared) = configured_env(&root);
        let paths = AppPaths::discover().unwrap();
        let session_path = populate_fixture(&root, &paths, &["a", "b"]);
        if label == "unrelated" {
            let mut session = fs::OpenOptions::new()
                .append(true)
                .open(&session_path)
                .unwrap();
            writeln!(
                session,
                "{{\"type\":\"thread\",\"payload\":{{\"thread_id\":\"actual-thread\"}}}}"
            )
            .unwrap();
        }
        let connection =
            rusqlite::Connection::open(paths.shared_codex_root.join("goals_1.sqlite")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE thread_goals (
                    thread_id TEXT PRIMARY KEY,
                    goal_id TEXT NOT NULL,
                    objective TEXT NOT NULL,
                    status TEXT NOT NULL,
                    token_budget INTEGER,
                    tokens_used INTEGER NOT NULL DEFAULT 0,
                    time_used_seconds INTEGER NOT NULL DEFAULT 0,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO thread_goals (thread_id, goal_id, objective, status, created_at_ms, updated_at_ms) VALUES (?1, 'goal-1', 'finish work', ?2, 1, 1)",
                rusqlite::params![thread_id, status],
            )
            .unwrap();
        drop(connection);

        let mut monitor = prepare_goal_usage_limit_monitor(
            &[OsString::from("exec"), OsString::from("work")],
            false,
        )
        .unwrap()
        .unwrap();
        super::super::goal_resume::write_runtime_goal_session_marker(
            &monitor.marker_path,
            std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
        )
        .unwrap();
        append_usage_limit(&session_path);
        let attempted = BTreeSet::new();
        let options = resume_options(&attempted, false, Some("a"));
        assert_eq!(
            plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
                .unwrap()
                .as_ref()
                .map(|plan| plan.profile_name.as_str()),
            expected_profile,
            "{label}"
        );
        drop(monitor);
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn exact_post_child_usage_limit_rotates_and_old_bytes_are_not_replayed() {
    let root = fixture_root("exact");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = prepare_runtime_usage_limit_monitor(
        &[OsString::from("exec"), OsString::from("work")],
        false,
    )
    .unwrap()
    .unwrap();
    super::super::goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
    )
    .unwrap();
    append_user_event(&session_path);
    append_usage_limit(&session_path);
    let attempted = BTreeSet::new();
    let options = resume_options(&attempted, false, Some("a"));

    let plan = plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
        .unwrap()
        .unwrap();
    assert_eq!(plan.profile_name, "b");

    monitor.prepare_for_resume();
    assert!(monitor.detect_usage_limit_after_child().unwrap().is_none());
    append_user_event(&session_path);
    append_usage_limit(&session_path);
    assert_eq!(
        monitor.detect_usage_limit_after_child().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn compressed_post_child_usage_limit_uses_decoded_marker_offset() {
    let root = fixture_root("compressed");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let compressed_path = session_path.with_extension("jsonl.zst");
    let initial = fs::read(&session_path).unwrap();
    fs::remove_file(&session_path).unwrap();
    fs::write(
        &compressed_path,
        zstd::stream::encode_all(initial.as_slice(), 3).unwrap(),
    )
    .unwrap();

    let mut monitor = prepare_runtime_usage_limit_monitor(
        &[OsString::from("exec"), OsString::from("work")],
        false,
    )
    .unwrap()
    .unwrap();
    super::super::goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
    )
    .unwrap();
    append_compressed_usage_limit(&compressed_path);

    assert_eq!(
        monitor.detect_usage_limit_after_child().unwrap().as_deref(),
        Some(SESSION_ID)
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn recovery_preserves_compacted_rollout_and_completed_tool_side_effect_once() {
    let root = fixture_root("compaction-side-effect");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = prepare_runtime_usage_limit_monitor(
        &[OsString::from("exec"), OsString::from("work")],
        false,
    )
    .unwrap()
    .unwrap();
    super::super::goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
    )
    .unwrap();

    let completed_progress = concat!(
        "{\"type\":\"compacted\",\"payload\":{\"window_id\":\"window-2\"}}\n",
        "{\"type\":\"tool_completed\",\"payload\":{\"call_id\":\"side-effect-1\"}}\n"
    );
    let mut session = fs::OpenOptions::new()
        .append(true)
        .open(&session_path)
        .unwrap();
    session.write_all(completed_progress.as_bytes()).unwrap();
    append_usage_limit(&session_path);
    let before_resume = fs::read(&session_path).unwrap();
    let attempted = BTreeSet::new();
    let options = resume_options(&attempted, false, Some("a"));

    let plan = plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
        .unwrap()
        .unwrap();
    assert_eq!(plan.profile_name, "b");

    let after_plan = fs::read(&session_path).unwrap();
    assert_eq!(after_plan, before_resume);
    assert_eq!(
        String::from_utf8_lossy(&after_plan)
            .matches("side-effect-1")
            .count(),
        1
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn usage_limit_recovery_reaches_a_late_ready_profile() {
    let root = fixture_root("late");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b", "c", "d"]);
    let mut monitor = prepare_runtime_usage_limit_monitor(
        &[OsString::from("exec"), OsString::from("work")],
        false,
    )
    .unwrap()
    .unwrap();
    super::super::goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
    )
    .unwrap();
    append_user_event(&session_path);
    append_usage_limit(&session_path);
    let attempted = BTreeSet::from(["b".to_string(), "c".to_string()]);
    let options = resume_options(&attempted, false, Some("a"));
    assert_eq!(
        plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
            .unwrap()
            .unwrap()
            .profile_name,
        "d"
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sequential_usage_limit_recovery_moves_from_b_to_c() {
    let root = fixture_root("sequential");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let _session_path = populate_fixture(&root, &paths, &["a", "b", "c"]);
    let mut state = AppState::load_and_repair(&paths).unwrap();
    let first_attempts = BTreeSet::new();
    let first = next_runtime_usage_limit_plan(
        &state,
        SESSION_ID,
        &resume_options(&first_attempts, false, Some("a")),
    )
    .unwrap();
    assert_eq!(first.profile_name, "b");
    assert_eq!(first.failed_profile_name, "a");

    let second_attempts = BTreeSet::from([first.failed_profile_name, first.profile_name.clone()]);
    state
        .session_profile_bindings
        .get_mut(SESSION_ID)
        .unwrap()
        .profile_name = first.profile_name;
    let second = next_runtime_usage_limit_plan(
        &state,
        SESSION_ID,
        &resume_options(&second_attempts, false, Some("b")),
    )
    .unwrap();
    assert_eq!(second.profile_name, "c");
    assert_eq!(second.failed_profile_name, "b");
    assert_eq!(second.session_id, SESSION_ID);
    let all_attempts = BTreeSet::from([
        "a".to_string(),
        "b".to_string(),
        second.profile_name.clone(),
    ]);
    state
        .session_profile_bindings
        .get_mut(SESSION_ID)
        .unwrap()
        .profile_name = second.profile_name;
    assert!(
        next_runtime_usage_limit_plan(
            &state,
            SESSION_ID,
            &resume_options(&all_attempts, false, Some("c")),
        )
        .is_none()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn no_auto_rotate_keeps_usage_limit_terminal() {
    let root = fixture_root("disabled");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let _session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = prepare_runtime_usage_limit_monitor(
        &[OsString::from("exec"), OsString::from("work")],
        false,
    )
    .unwrap()
    .unwrap();
    let attempted = BTreeSet::new();
    let options = resume_options(&attempted, true, Some("a"));
    assert!(
        plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(1), &options)
            .unwrap()
            .is_none()
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn explicit_cancellation_does_not_resume_a_pending_session() {
    let root = fixture_root("cancelled");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let mut monitor = prepare_runtime_usage_limit_monitor(
        &[OsString::from("exec"), OsString::from("work")],
        false,
    )
    .unwrap()
    .unwrap();
    super::super::goal_resume::write_runtime_goal_session_marker(
        &monitor.marker_path,
        std::ffi::OsStr::new(&format!(r#"{{"session_id":"{SESSION_ID}"}}"#)),
    )
    .unwrap();
    append_usage_limit(&session_path);
    let attempted = BTreeSet::new();
    let options = resume_options(&attempted, false, Some("a"));

    assert!(
        plan_runtime_usage_limit_relaunch(&mut monitor, &exit_status(130), &options)
            .unwrap()
            .is_none()
    );
    drop(monitor);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_recovery_telemetry_uses_observed_state_without_raw_profile_identity() {
    let _runtime_lock = crate::acquire_test_runtime_lock();
    let root = fixture_root("telemetry");
    let log_path = root.join("prodex-runtime-recovery.log");
    crate::open_runtime_proxy_private_file(&log_path).unwrap();
    let plan = GoalResumeRelaunchPlan {
        session_id: SESSION_ID.to_string(),
        failed_profile_name: "profile-a".to_string(),
        profile_name: "profile-b".to_string(),
        failure_class: "transport",
        resume_goal: false,
        evidence: RuntimeWorkflowEvidence {
            acceptance_state: "side_effect_observed",
            stream_committed: Some(true),
            side_effect_state: "observed",
        },
    };

    crate::runtime_proxy_log_to_path(
        &log_path,
        &runtime_session_recovery_message(&plan, 2, Some("gpt-5.6-luna"), Some("gpt-5.2")),
    );
    crate::runtime_proxy_flush_logs_for_path(&log_path).unwrap();
    let log = fs::read_to_string(&log_path).unwrap();
    for expected in [
        "retry_layer=session",
        "recovery_generation=2",
        "failure_class=transport",
        "requested_model=gpt-5.6-luna",
        "effective_model=gpt-5.2",
        "acceptance_state=side_effect_observed",
        "stream_committed=true",
        "side_effect_state=observed",
        "last_prompt_requeued=false",
    ] {
        assert!(log.contains(expected), "missing {expected}: {log}");
    }
    assert!(!log.contains("profile-b"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn large_historical_rollout_keeps_incremental_monitor_available() {
    let root = fixture_root("oversized-monitor");
    let (_home, _shared) = configured_env(&root);
    let paths = AppPaths::discover().unwrap();
    let session_path = populate_fixture(&root, &paths, &["a", "b"]);
    let offset = 65 * 1024 * 1024;
    fs::OpenOptions::new()
        .write(true)
        .open(&session_path)
        .unwrap()
        .set_len(offset)
        .unwrap();
    let marker = root.join("monitor.id");
    fs::write(&marker, format!("{SESSION_ID}\n")).unwrap();
    let mut monitor = GoalUsageLimitMonitor::new(paths, None, marker, Some(SESSION_ID.to_string()));
    monitor.session_path = Some(session_path);
    monitor.session_scan_offset = offset;

    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    assert!(!monitor.workflow_scan_disabled);
    assert_eq!(monitor.take_usage_limit_signal().unwrap(), None);
    let _ = fs::remove_dir_all(root);
}
