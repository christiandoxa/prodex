use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const AUDIT_LOG_FILE_NAME: &str = "prodex-audit.log";
const AUDIT_LOG_READ_MAX_BYTES: u64 = 512 * 1024;

#[derive(Debug, Serialize)]
struct AuditEvent<'a> {
    recorded_at: String,
    recorded_at_epoch: i64,
    pid: u32,
    component: &'a str,
    action: &'a str,
    outcome: &'a str,
    details: Value,
}

#[derive(Debug, Clone, Default)]
pub(super) struct AuditLogQuery {
    pub(super) tail: usize,
    pub(super) component: Option<String>,
    pub(super) action: Option<String>,
    pub(super) outcome: Option<String>,
}

impl AuditLogQuery {
    pub(super) fn has_filters(&self) -> bool {
        self.component.is_some() || self.action.is_some() || self.outcome.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct AuditLogEventRecord {
    pub(super) recorded_at: String,
    pub(super) recorded_at_epoch: i64,
    pub(super) pid: u32,
    pub(super) component: String,
    pub(super) action: String,
    pub(super) outcome: String,
    pub(super) details: Value,
}

pub(super) fn audit_log_dir() -> PathBuf {
    env::var_os("PRODEX_AUDIT_LOG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(super::runtime_proxy_log_dir)
}

pub(super) fn audit_log_path() -> PathBuf {
    audit_log_dir().join(AUDIT_LOG_FILE_NAME)
}

pub(super) fn append_audit_event(
    component: &str,
    action: &str,
    outcome: &str,
    details: Value,
) -> Result<()> {
    let path = audit_log_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let event = AuditEvent {
        recorded_at: Local::now().to_rfc3339(),
        recorded_at_epoch: Local::now().timestamp(),
        pid: std::process::id(),
        component,
        action,
        outcome,
        details,
    };
    let line = serde_json::to_string(&event).context("failed to serialize audit event")?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    writeln!(file, "{line}").with_context(|| format!("failed to append {}", path.display()))?;
    Ok(())
}

pub(super) fn format_audit_logs_summary() -> String {
    let path = audit_log_path();
    format!(
        "{} ({})",
        path.display(),
        if path.exists() { "exists" } else { "missing" }
    )
}

pub(super) fn audit_logs_json_value() -> Value {
    let path = audit_log_path();
    serde_json::json!({
        "directory": audit_log_dir().display().to_string(),
        "path": path.display().to_string(),
        "exists": path.exists(),
    })
}

pub(super) fn read_recent_audit_events(query: &AuditLogQuery) -> Result<Vec<AuditLogEventRecord>> {
    let path = audit_log_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let candidate_lines = if query.has_filters() {
        read_recent_audit_lines(&path, None)?
    } else {
        read_recent_audit_lines(&path, Some(query.tail))?
    };
    let mut matches = Vec::new();
    for line in candidate_lines {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<AuditLogEventRecord>(&line) else {
            continue;
        };
        if audit_log_query_matches(query, &event) {
            matches.push(event);
        }
    }

    if matches.len() > query.tail {
        let keep_from = matches.len().saturating_sub(query.tail);
        Ok(matches.split_off(keep_from))
    } else {
        Ok(matches)
    }
}

fn audit_log_query_matches(query: &AuditLogQuery, event: &AuditLogEventRecord) -> bool {
    let component_matches = query
        .component
        .as_deref()
        .is_none_or(|component| event.component == component);
    let action_matches = query
        .action
        .as_deref()
        .is_none_or(|action| event.action == action);
    let outcome_matches = query
        .outcome
        .as_deref()
        .is_none_or(|outcome| event.outcome == outcome);
    component_matches && action_matches && outcome_matches
}

pub(super) fn render_audit_events_human(
    path: &Path,
    query: &AuditLogQuery,
    events: &[AuditLogEventRecord],
) -> String {
    let mut output = String::new();
    output.push_str(&format!("Audit log: {}\n", path.display()));
    output.push_str(&format!("Tail: {}\n", query.tail));
    if query.has_filters() {
        output.push_str(&format!("Filter: {}\n", format_audit_query(query)));
    }

    if events.is_empty() {
        output.push_str("No matching audit events.");
        return output;
    }

    output.push_str(&format!("Events: {}\n", events.len()));
    for event in events {
        output.push_str(&format!(
            "{}  {} {} {} {}\n",
            event.recorded_at,
            event.component,
            event.action,
            event.outcome,
            summarize_audit_details(&event.details),
        ));
    }
    output.pop();
    output
}

pub(super) fn format_audit_query(query: &AuditLogQuery) -> String {
    let mut parts = Vec::new();
    if let Some(component) = query.component.as_deref() {
        parts.push(format!("component={component}"));
    }
    if let Some(action) = query.action.as_deref() {
        parts.push(format!("action={action}"));
    }
    if let Some(outcome) = query.outcome.as_deref() {
        parts.push(format!("outcome={outcome}"));
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(" ")
    }
}

fn summarize_audit_details(details: &Value) -> String {
    let rendered = match details {
        Value::Null => "-".to_string(),
        _ => serde_json::to_string(details).unwrap_or_else(|_| "<invalid-json>".to_string()),
    };
    truncate_audit_details(&rendered, 160)
}

fn truncate_audit_details(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn read_recent_audit_lines(path: &Path, tail: Option<usize>) -> Result<Vec<String>> {
    if tail == Some(0) {
        return Ok(Vec::new());
    }
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len();
    let start = len.saturating_sub(AUDIT_LOG_READ_MAX_BYTES);
    let mut reader = BufReader::new(file);
    reader
        .seek(SeekFrom::Start(start))
        .with_context(|| format!("failed to seek {}", path.display()))?;
    let mut content = String::new();
    reader
        .read_to_string(&mut content)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut lines = content.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    if start > 0 && !content.starts_with('\n') && !lines.is_empty() {
        lines.remove(0);
    }
    if let Some(tail) = tail
        && lines.len() > tail
    {
        let keep_from = lines.len().saturating_sub(tail);
        return Ok(lines.split_off(keep_from));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
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
}
