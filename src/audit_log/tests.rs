use super::*;
use crate::TestEnvVarGuard;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = env::temp_dir().join(format!(
        "prodex-audit-log-{name}-{}-{nanos:x}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn audit_event_line(epoch: i64, component: &str, action: &str, details: Value) -> String {
    format!(
        "{{\"recorded_at\":\"2026-04-08T00:00:00+00:00\",\"recorded_at_epoch\":{epoch},\"pid\":10,\"component\":\"{component}\",\"action\":\"{action}\",\"outcome\":\"success\",\"details\":{details}}}\n",
    )
}

#[test]
fn audit_log_path_uses_env_override() {
    let dir = temp_dir("path");
    let _guard = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());

    assert_eq!(audit_log_path(), dir.join(AUDIT_LOG_FILE_NAME));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn append_audit_event_writes_json_line() {
    let dir = temp_dir("append");
    let _guard = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());

    append_audit_event(
        "profile",
        "add",
        "success",
        serde_json::json!({
            "profile_name": "main",
            "managed": true,
        }),
    )
    .unwrap();

    let content = fs::read_to_string(audit_log_path()).unwrap();
    let line = content.lines().next().unwrap();
    let value: Value = serde_json::from_str(line).unwrap();
    assert_eq!(
        value.get("component").and_then(Value::as_str),
        Some("profile")
    );
    assert_eq!(value.get("action").and_then(Value::as_str), Some("add"));
    assert_eq!(
        value.get("outcome").and_then(Value::as_str),
        Some("success")
    );
    assert_eq!(
        value
            .pointer("/details/profile_name")
            .and_then(Value::as_str),
        Some("main")
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn read_recent_audit_events_applies_tail_and_filters() {
    let dir = temp_dir("query");
    let _guard = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());

    fs::write(
        audit_log_path(),
        concat!(
            "{\"recorded_at\":\"2026-04-08T00:00:00+00:00\",\"recorded_at_epoch\":1,\"pid\":10,\"component\":\"profile\",\"action\":\"add\",\"outcome\":\"success\",\"details\":{\"profile_name\":\"main\"}}\n",
            "not-json\n",
            "{\"recorded_at\":\"2026-04-08T00:00:01+00:00\",\"recorded_at_epoch\":2,\"pid\":10,\"component\":\"profile\",\"action\":\"use\",\"outcome\":\"success\",\"details\":{\"profile_name\":\"second\"}}\n",
            "{\"recorded_at\":\"2026-04-08T00:00:02+00:00\",\"recorded_at_epoch\":3,\"pid\":10,\"component\":\"runtime\",\"action\":\"broker_start\",\"outcome\":\"success\",\"details\":{\"listen_addr\":\"127.0.0.1:12345\"}}\n"
        ),
    )
    .unwrap();

    let filtered = read_recent_audit_events(&AuditLogQuery {
        tail: 5,
        component: Some("profile".to_string()),
        action: None,
        outcome: Some("success".to_string()),
    })
    .unwrap();
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].action, "add");
    assert_eq!(filtered[1].action, "use");

    let tailed = read_recent_audit_events(&AuditLogQuery {
        tail: 1,
        component: None,
        action: None,
        outcome: None,
    })
    .unwrap();
    assert_eq!(tailed.len(), 1);
    assert_eq!(tailed[0].component, "runtime");
    assert_eq!(tailed[0].action, "broker_start");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn read_recent_audit_events_with_filters_scans_beyond_last_tail_lines() {
    let dir = temp_dir("query-filter-window");
    let _guard = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());

    let mut content = String::new();
    content.push_str(
        "{\"recorded_at\":\"2026-04-08T00:00:00+00:00\",\"recorded_at_epoch\":1,\"pid\":10,\"component\":\"profile\",\"action\":\"use\",\"outcome\":\"success\",\"details\":{\"profile_name\":\"main\"}}\n",
    );
    for index in 0..20 {
        content.push_str(&format!(
            "{{\"recorded_at\":\"2026-04-08T00:00:{:02}+00:00\",\"recorded_at_epoch\":{},\"pid\":10,\"component\":\"runtime\",\"action\":\"broker_start\",\"outcome\":\"success\",\"details\":{{\"index\":{}}}}}\n",
            index + 1,
            index + 2,
            index
        ));
    }
    fs::write(audit_log_path(), content).unwrap();

    let events = read_recent_audit_events(&AuditLogQuery {
        tail: 5,
        component: Some("profile".to_string()),
        action: Some("use".to_string()),
        outcome: Some("success".to_string()),
    })
    .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].component, "profile");
    assert_eq!(events[0].action, "use");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn read_recent_audit_events_with_scope_reports_limited_search_window() {
    let dir = temp_dir("query-limited-window");
    let _guard = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());

    let mut content = String::new();
    content.push_str(&audit_event_line(
        1,
        "profile",
        "use",
        serde_json::json!({"profile_name":"outside-window"}),
    ));
    let filler_line = audit_event_line(2, "runtime", "broker_start", serde_json::json!({}));
    while content.len() <= (AUDIT_LOG_READ_MAX_BYTES as usize + filler_line.len()) {
        content.push_str(&filler_line);
    }
    fs::write(audit_log_path(), &content).unwrap();

    let result = read_recent_audit_events_with_scope(&AuditLogQuery {
        tail: 5,
        component: Some("profile".to_string()),
        action: Some("use".to_string()),
        outcome: Some("success".to_string()),
    })
    .unwrap();

    assert!(result.events.is_empty());
    assert_eq!(result.search_scope.path, audit_log_path());
    assert_eq!(result.search_scope.log_size_bytes, content.len() as u64);
    assert_eq!(result.search_scope.searched_bytes, AUDIT_LOG_READ_MAX_BYTES);
    assert_eq!(
        result.search_scope.search_start_byte,
        result.search_scope.log_size_bytes - AUDIT_LOG_READ_MAX_BYTES
    );
    assert_eq!(
        result.search_scope.read_limit_bytes,
        AUDIT_LOG_READ_MAX_BYTES
    );
    assert!(result.search_scope.limited);

    let rendered_scope = format_audit_search_scope(&result.search_scope);
    assert!(rendered_scope.contains("limited to last 524288 bytes"));

    let output = render_audit_events_human_with_scope(
        &result.search_scope.path,
        &AuditLogQuery {
            tail: 5,
            component: Some("profile".to_string()),
            action: Some("use".to_string()),
            outcome: Some("success".to_string()),
        },
        &result.events,
        Some(&result.search_scope),
    );
    assert!(output.contains("Search scope: searched 524288"));
    assert!(output.contains("No matching audit events."));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn read_recent_audit_events_with_scope_reports_tail_zero_empty_search() {
    let dir = temp_dir("query-tail-zero");
    let _guard = TestEnvVarGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());
    let content = audit_event_line(1, "profile", "use", serde_json::json!({}));
    fs::write(audit_log_path(), &content).unwrap();

    let result = read_recent_audit_events_with_scope(&AuditLogQuery {
        tail: 0,
        component: None,
        action: None,
        outcome: None,
    })
    .unwrap();

    assert!(result.events.is_empty());
    assert_eq!(result.search_scope.log_size_bytes, content.len() as u64);
    assert_eq!(result.search_scope.search_start_byte, content.len() as u64);
    assert_eq!(result.search_scope.searched_bytes, 0);
    assert!(!result.search_scope.limited);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn render_audit_events_human_shows_filters_and_details() {
    let output = render_audit_events_human(
        Path::new("/tmp/prodex-audit.log"),
        &AuditLogQuery {
            tail: 25,
            component: Some("profile".to_string()),
            action: None,
            outcome: Some("success".to_string()),
        },
        &[AuditLogEventRecord {
            recorded_at: "2026-04-08T00:00:00+00:00".to_string(),
            recorded_at_epoch: 1,
            pid: 10,
            component: "profile".to_string(),
            action: "add".to_string(),
            outcome: "success".to_string(),
            details: serde_json::json!({"profile_name":"main"}),
        }],
    );

    assert!(output.contains("Tail: 25"));
    assert!(output.contains("Filter: component=profile outcome=success"));
    assert!(output.contains("profile add success"));
    assert!(output.contains("\"profile_name\":\"main\""));
}
