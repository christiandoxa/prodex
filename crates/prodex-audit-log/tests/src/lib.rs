use super::*;
use std::io::{Cursor, Write};
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
fn audit_log_dir_uses_injected_override() {
    let dir = temp_dir("path");
    let fallback = temp_dir("fallback");

    assert_eq!(
        audit_log_dir_with_override(&fallback, Some(dir.clone().into_os_string())),
        dir
    );

    let _ = fs::remove_dir_all(dir);
    let _ = fs::remove_dir_all(fallback);
}

#[test]
fn append_audit_event_writes_json_line() {
    let dir = temp_dir("append");
    let path = dir.join(AUDIT_LOG_FILE_NAME);

    append_audit_event(
        &path,
        "profile",
        "add",
        "success",
        serde_json::json!({
            "profile_name": "main",
            "managed": true,
        }),
    )
    .unwrap();

    let content = fs::read_to_string(&path).unwrap();
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
    let path = dir.join(AUDIT_LOG_FILE_NAME);

    fs::write(
            &path,
            concat!(
                "{\"recorded_at\":\"2026-04-08T00:00:00+00:00\",\"recorded_at_epoch\":1,\"pid\":10,\"component\":\"profile\",\"action\":\"add\",\"outcome\":\"success\",\"details\":{\"profile_name\":\"main\"}}\n",
                "{\"recorded_at\":\"2026-04-08T00:00:01+00:00\",\"recorded_at_epoch\":2,\"pid\":10,\"component\":\"profile\",\"action\":\"use\",\"outcome\":\"success\",\"details\":{\"profile_name\":\"second\"}}\n",
                "{\"recorded_at\":\"2026-04-08T00:00:02+00:00\",\"recorded_at_epoch\":3,\"pid\":10,\"component\":\"runtime\",\"action\":\"broker_start\",\"outcome\":\"success\",\"details\":{\"listen_addr\":\"127.0.0.1:12345\"}}\n"
            ),
        )
        .unwrap();

    let filtered = read_recent_audit_events(
        &path,
        &AuditLogQuery {
            tail: 5,
            component: Some("profile".to_string()),
            action: None,
            outcome: Some("success".to_string()),
        },
    )
    .unwrap();
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].action, "add");
    assert_eq!(filtered[1].action, "use");

    let tailed = read_recent_audit_events(
        &path,
        &AuditLogQuery {
            tail: 1,
            component: None,
            action: None,
            outcome: None,
        },
    )
    .unwrap();
    assert_eq!(tailed.len(), 1);
    assert_eq!(tailed[0].component, "runtime");
    assert_eq!(tailed[0].action, "broker_start");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn read_recent_audit_events_with_filters_scans_beyond_last_tail_lines() {
    let dir = temp_dir("query-filter-window");
    let path = dir.join(AUDIT_LOG_FILE_NAME);

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
    fs::write(&path, content).unwrap();

    let events = read_recent_audit_events(
        &path,
        &AuditLogQuery {
            tail: 5,
            component: Some("profile".to_string()),
            action: Some("use".to_string()),
            outcome: Some("success".to_string()),
        },
    )
    .unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].component, "profile");
    assert_eq!(events[0].action, "use");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn read_recent_audit_events_with_scope_reports_limited_search_window() {
    let dir = temp_dir("query-limited-window");
    let path = dir.join(AUDIT_LOG_FILE_NAME);

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
    fs::write(&path, &content).unwrap();

    let result = read_recent_audit_events_with_scope(
        &path,
        &AuditLogQuery {
            tail: 5,
            component: Some("profile".to_string()),
            action: Some("use".to_string()),
            outcome: Some("success".to_string()),
        },
    )
    .unwrap();

    assert!(result.events.is_empty());
    assert_eq!(result.search_scope.path, path);
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
    let path = dir.join(AUDIT_LOG_FILE_NAME);
    let content = audit_event_line(1, "profile", "use", serde_json::json!({}));
    fs::write(&path, &content).unwrap();

    let result = read_recent_audit_events_with_scope(
        &path,
        &AuditLogQuery {
            tail: 0,
            component: None,
            action: None,
            outcome: None,
        },
    )
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
            schema_version: 0,
            recorded_at: "2026-04-08T00:00:00+00:00".to_string(),
            recorded_at_epoch: 1,
            pid: 10,
            component: "profile".to_string(),
            action: "add".to_string(),
            outcome: "success".to_string(),
            details: serde_json::json!({"profile_name":"main"}),
            checksum: None,
        }],
    );

    assert!(output.contains("Tail: 25"));
    assert!(output.contains("Filter: component=profile outcome=success"));
    assert!(output.contains("profile add success"));
    assert!(output.contains("\"profile_name\":\"main\""));
}

fn usage_row(epoch: i64, tokens: u64, cost_micros: u64) -> UsageLedgerRow {
    UsageLedgerRow {
        recorded_at_epoch: epoch,
        source: "runtime-proxy".to_string(),
        operation: "responses".to_string(),
        outcome: "success".to_string(),
        model: Some("gpt-5.3-codex".to_string()),
        request_id: Some("request-1".to_string()),
        metadata: UsageLedgerMetadata::redacted(
            Some("main"),
            Some("acct-sensitive-1234"),
            Some("User@Example.COM"),
        ),
        input_tokens: tokens / 2,
        output_tokens: tokens / 2,
        cached_input_tokens: 3,
        reasoning_tokens: 0,
        total_tokens: tokens,
        cost_micros,
    }
}

#[test]
fn usage_ledger_row_serialization_redacts_account_metadata() {
    let row = usage_row(100, 10, 5);
    let line = usage_ledger_row_to_json_line(&row).unwrap();

    assert!(line.contains("\"profile_name\":\"main\""));
    assert!(line.contains("\"account_hint\":\"...1234\""));
    assert!(line.contains("\"email_domain\":\"example.com\""));
    assert!(!line.contains("acct-sensitive"));
    assert!(!line.contains("User@Example"));

    let parsed = parse_usage_ledger_line(&line).expect("usage row parsed");
    assert_eq!(parsed.metadata.account_hint.as_deref(), Some("...1234"));
    assert_eq!(parsed.metadata.email_domain.as_deref(), Some("example.com"));
}

#[test]
fn usage_ledger_read_rejects_malformed_lines() {
    let dir = temp_dir("usage-ledger");
    let path = dir.join(USAGE_LEDGER_FILE_NAME);

    append_usage_ledger_row(&path, &usage_row(100, 10, 5)).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"not-json\n\n")
        .unwrap();
    let error = read_usage_ledger_rows(&path).unwrap_err();
    assert!(error.to_string().contains("line 2"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn usage_ledger_read_rejects_oversized_file() {
    let dir = temp_dir("usage-ledger-oversized");
    let path = dir.join(USAGE_LEDGER_FILE_NAME);
    fs::File::create(&path)
        .unwrap()
        .set_len(USAGE_LEDGER_READ_MAX_BYTES + 1)
        .unwrap();

    let error = read_usage_ledger_rows(&path).unwrap_err();
    assert!(error.to_string().contains("bounded read limit"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn bounded_utf8_reader_rejects_growth_past_limit() {
    let error = read_utf8_bounded(Cursor::new(b"123456789"), 8, "audit log").unwrap_err();

    assert!(error.to_string().contains("bounded read limit"));
}

#[test]
fn append_audit_event_redacts_secrets_and_secures_file() {
    let dir = temp_dir("append-private");
    let path = dir.join(AUDIT_LOG_FILE_NAME);

    append_audit_event(
        &path,
        "runtime",
        "request",
        "failure",
        serde_json::json!({
            "authorization": "Bearer secret-token",
            "nested": {"api_key": "sensitive"},
        }),
    )
    .unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.contains("secret-token"));
    assert!(!content.contains("sensitive"));
    assert!(content.contains("<redacted>"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn audit_reader_rejects_corrupt_checksummed_record() {
    let dir = temp_dir("audit-corrupt");
    let path = dir.join(AUDIT_LOG_FILE_NAME);
    append_audit_event(
        &path,
        "profile",
        "add",
        "success",
        serde_json::json!({"profile_name": "main"}),
    )
    .unwrap();

    let content = fs::read_to_string(&path).unwrap();
    fs::write(&path, content.replace("\"main\"", "\"tampered\"")).unwrap();

    let error = read_recent_audit_events(
        &path,
        &AuditLogQuery {
            tail: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("checksum mismatch"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn concurrent_audit_appends_keep_complete_records() {
    let dir = temp_dir("audit-concurrent");
    let path = dir.join(AUDIT_LOG_FILE_NAME);
    let threads = (0..8)
        .map(|index| {
            let path = path.clone();
            std::thread::spawn(move || {
                append_audit_event(
                    &path,
                    "runtime",
                    "request",
                    "success",
                    serde_json::json!({"index": index}),
                )
                .unwrap();
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }

    let events = read_recent_audit_events(
        &path,
        &AuditLogQuery {
            tail: 8,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(events.len(), 8);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn usage_budget_evaluation_blocks_when_limits_are_reached() {
    let rows = vec![usage_row(3_600, 10, 5), usage_row(3_700, 20, 10)];
    let limit = BudgetLimit {
        key: "Project A".to_string(),
        window: BudgetWindow::Hour,
        max_requests: Some(2),
        max_tokens: Some(100),
        max_cost_micros: None,
    };

    let evaluation = evaluate_budget_limit(&limit, &rows, 3_800);

    assert_eq!(evaluation.key, "project-a");
    assert!(!evaluation.allowed);
    assert_eq!(evaluation.summary.requests, 2);
    assert_eq!(evaluation.summary.total_tokens, 30);
    assert_eq!(
        evaluation.reasons,
        vec!["request limit reached (2/2)".to_string()]
    );
}

#[test]
fn usage_summary_uses_selected_window() {
    let rows = vec![
        usage_row(3_500, 10, 5),
        usage_row(3_600, 20, 10),
        usage_row(3_700, 30, 20),
    ];

    let summary = summarize_usage_for_window(&rows, BudgetWindow::Hour, 3_800);

    assert_eq!(summary.since_epoch, 3_600);
    assert_eq!(summary.requests, 2);
    assert_eq!(summary.total_tokens, 50);
    assert_eq!(summary.cost_micros, 30);
}
