use super::*;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

mod large_files;
mod repair;
mod state_db;

fn assert_codex_session_meta_line(
    line: &str,
    session_id: &str,
    expected_timestamp: Option<&str>,
) -> serde_json::Value {
    let value: serde_json::Value =
        serde_json::from_str(line).expect("synthetic metadata should be valid JSON");
    assert_eq!(value["type"], "session_meta");
    assert_eq!(value["payload"]["session_id"], session_id);
    assert_eq!(value["payload"]["id"], session_id);
    let outer_timestamp = value["timestamp"]
        .as_str()
        .expect("synthetic metadata should include rollout timestamp");
    let payload_timestamp = value["payload"]["timestamp"]
        .as_str()
        .expect("synthetic metadata should include session timestamp");
    assert_eq!(outer_timestamp, payload_timestamp);
    if let Some(expected_timestamp) = expected_timestamp {
        assert_eq!(outer_timestamp, expected_timestamp);
    }
    assert_eq!(value["payload"]["originator"], "prodex-repair");
    assert_eq!(value["payload"]["cli_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["payload"]["source"], "cli");
    let cwd = value["payload"]["cwd"]
        .as_str()
        .expect("synthetic metadata should include cwd");
    let _: PathBuf = serde_json::from_value(value["payload"]["cwd"].clone())
        .expect("synthetic cwd should deserialize as a Codex PathBuf");
    assert!(!cwd.is_empty());
    value
}

#[test]
fn session_current_filters_by_cwd() {
    let root = test_temp_dir("session-current");
    let sessions = root.join("sessions");
    let current = root.join("current");
    let other = root.join("other");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::create_dir_all(&current).expect("current dir should be created");
    fs::create_dir_all(&other).expect("other dir should be created");
    fs::write(
        sessions.join("current.jsonl"),
        serde_json::json!({
            "timestamp": "2026-04-29T12:00:00Z",
            "payload": { "id": "current", "cwd": current.as_path() }
        })
        .to_string()
            + "\n",
    )
    .expect("current session should be written");
    fs::write(
        sessions.join("other.jsonl"),
        serde_json::json!({
            "timestamp": "2026-04-29T12:00:00Z",
            "payload": { "id": "other", "cwd": other.as_path() }
        })
        .to_string()
            + "\n",
    )
    .expect("other session should be written");

    let reports = collect_session_reports(&root, Some(&current), &AppState::default())
        .expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "current");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_exclude_subagents_by_default() {
    let root = test_temp_dir("session-subagents");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::write(
        sessions.join("parent.jsonl"),
        "{\"timestamp\":\"2026-04-29T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"parent\",\"cwd\":\"/tmp/workspace\"}}\n",
    )
    .expect("parent session should be written");
    fs::write(
        sessions.join("child.jsonl"),
        "{\"timestamp\":\"2026-04-29T12:01:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"child\",\"cwd\":\"/tmp/workspace\",\"source\":{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"parent\"}}}}}\n",
    )
    .expect("child session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "parent");

    let reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            include_subagents: true,
            ..SessionReportFilter::default()
        },
        &AppState::default(),
    )
    .expect("sessions collect");
    assert_eq!(reports.len(), 2);
    assert!(reports.iter().any(|report| report.id == "child"));
    assert_eq!(
        reports
            .iter()
            .find(|report| report.id == "child")
            .and_then(|report| report.parent_thread_id.as_deref()),
        Some("parent")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_attach_profile_bindings() {
    let root = test_temp_dir("session-profile");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::write(
        sessions.join("bound.jsonl"),
        "{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{\"id\":\"bound\"}}\n",
    )
    .expect("session should be written");
    let state = AppState {
        session_profile_bindings: BTreeMap::from([(
            "bound".to_string(),
            prodex_state::ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: 1,
            },
        )]),
        ..AppState::default()
    };

    let reports = collect_session_reports(&root, None, &state).expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].profile.as_deref(), Some("main"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_filter_by_profile_and_query() {
    let root = test_temp_dir("session-filter");
    let sessions = root.join("sessions");
    let alpha_cwd = root.join("WorkspaceAlpha");
    let beta_cwd = root.join("WorkspaceBeta");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::create_dir_all(&alpha_cwd).expect("alpha cwd should be created");
    fs::create_dir_all(&beta_cwd).expect("beta cwd should be created");
    fs::write(
        sessions.join("alpha-special-path.jsonl"),
        serde_json::json!({
            "timestamp": "2026-04-29T12:00:00Z",
            "payload": {
                "id": "alpha-session",
                "thread_name": "Issue Triage",
                "cwd": alpha_cwd.as_path()
            }
        })
        .to_string()
            + "\n",
    )
    .expect("alpha session should be written");
    fs::write(
        sessions.join("beta-special-path.jsonl"),
        serde_json::json!({
            "timestamp": "2026-04-29T12:00:00Z",
            "payload": {
                "id": "beta-session",
                "thread_name": "Docs Review",
                "cwd": beta_cwd.as_path()
            }
        })
        .to_string()
            + "\n",
    )
    .expect("beta session should be written");
    let state = AppState {
        session_profile_bindings: BTreeMap::from([
            (
                "alpha-session".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: 1,
                },
            ),
            (
                "beta-session".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "alt".to_string(),
                    bound_at: 1,
                },
            ),
        ]),
        ..AppState::default()
    };

    let profile_reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            current_dir: None,
            profile: Some("main"),
            query: Some("triage"),
            include_subagents: false,
        },
        &state,
    )
    .expect("sessions collect");
    assert_eq!(profile_reports.len(), 1);
    assert_eq!(profile_reports[0].id, "alpha-session");

    let cwd_reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            current_dir: None,
            profile: None,
            query: Some("workspacebeta"),
            include_subagents: false,
        },
        &state,
    )
    .expect("sessions collect");
    assert_eq!(cwd_reports.len(), 1);
    assert_eq!(cwd_reports[0].id, "beta-session");

    let path_reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            current_dir: None,
            profile: None,
            query: Some("ALPHA-SPECIAL-PATH"),
            include_subagents: false,
        },
        &state,
    )
    .expect("sessions collect");
    assert_eq!(path_reports.len(), 1);
    assert_eq!(path_reports[0].id, "alpha-session");

    let profile_query_reports = collect_session_reports_with_filter(
        &root,
        SessionReportFilter {
            current_dir: None,
            profile: None,
            query: Some("ALT"),
            include_subagents: false,
        },
        &state,
    )
    .expect("sessions collect");
    assert_eq!(profile_query_reports.len(), 1);
    assert_eq!(profile_query_reports[0].id, "beta-session");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_resolver_handles_unique_ambiguous_and_missing_ids() {
    let root = test_temp_dir("session-resolve");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let first_id = "11111111-1111-1111-1111-111111111111";
    let second_id = "11111111-1111-1111-1111-222222222222";
    fs::write(
        sessions.join("first.jsonl"),
        format!(
            "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"{first_id}\"}}}}\n"
        ),
    )
    .expect("first session should be written");
    fs::write(
        sessions.join("second.jsonl"),
        format!(
            "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"{second_id}\"}}}}\n"
        ),
    )
    .expect("second session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");
    assert_eq!(
        resolve_session_report_by_id(&reports, &first_id.to_uppercase())
            .expect("exact match should resolve")
            .id,
        first_id
    );
    assert_eq!(
        resolve_session_report_by_id(&reports, "11111111-1111-1111-1111-222")
            .expect("unique prefix should resolve")
            .id,
        second_id
    );
    assert!(matches!(
        resolve_session_report_by_id(&reports, "11111111").unwrap_err(),
        SessionResolveError::Ambiguous { .. }
    ));
    assert!(matches!(
        resolve_session_report_by_id(&reports, "99999999").unwrap_err(),
        SessionResolveError::Missing { .. }
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_resolve_error_response_is_stable_and_redacted() {
    let missing = SessionResolveError::Missing {
        selector: "99999999-secret-selector".to_string(),
    };
    let missing_response = plan_session_resolve_error_response(&missing);
    assert_eq!(missing_response.status, SessionResolveErrorStatus::NotFound);
    assert_eq!(missing_response.code, "session_not_found");
    assert_eq!(missing_response.message, "session could not be resolved");
    assert!(!missing_response.message.contains("99999999"));
    assert!(!missing_response.message.contains("secret"));
    let missing_display = missing.to_string();
    assert_eq!(missing_display, "session could not be resolved");
    assert!(!missing_display.contains("99999999"));
    assert!(!missing_display.contains("secret"));

    let ambiguous = SessionResolveError::Ambiguous {
        selector: "11111111".to_string(),
        matches: vec![
            "11111111-1111-1111-1111-111111111111".to_string(),
            "11111111-1111-1111-1111-222222222222".to_string(),
        ],
    };
    let ambiguous_response = plan_session_resolve_error_response(&ambiguous);
    assert_eq!(
        ambiguous_response.status,
        SessionResolveErrorStatus::Conflict
    );
    assert_eq!(ambiguous_response.code, "session_selector_ambiguous");
    assert_eq!(ambiguous_response.message, "session selector is ambiguous");
    assert!(!ambiguous_response.message.contains("11111111"));
    assert!(!ambiguous_response.message.contains("22222222"));
    let ambiguous_display = ambiguous.to_string();
    assert_eq!(ambiguous_display, "session selector is ambiguous");
    assert!(!ambiguous_display.contains("11111111"));
    assert!(!ambiguous_display.contains("22222222"));
}

fn test_temp_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "prodex-session-store-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp dir should be removed");
    }
    fs::create_dir_all(&root).expect("temp dir should be created");
    root
}

fn assert_no_repair_temporary_files(directory: &Path) {
    let entries = fs::read_dir(directory).expect("session directory should read");
    assert!(
        entries.filter_map(|entry| entry.ok()).all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .contains("prodex-repair-tmp")),
        "repair temporary files should be cleaned up"
    );
}
