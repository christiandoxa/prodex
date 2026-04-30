use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const AUDIT_LOG_FILE_NAME: &str = "prodex-audit.log";
pub const AUDIT_LOG_READ_MAX_BYTES: u64 = 512 * 1024;

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
pub struct AuditLogQuery {
    pub tail: usize,
    pub component: Option<String>,
    pub action: Option<String>,
    pub outcome: Option<String>,
}

impl AuditLogQuery {
    pub fn has_filters(&self) -> bool {
        self.component.is_some() || self.action.is_some() || self.outcome.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEventRecord {
    pub recorded_at: String,
    pub recorded_at_epoch: i64,
    pub pid: u32,
    pub component: String,
    pub action: String,
    pub outcome: String,
    pub details: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditLogSearchScope {
    pub path: PathBuf,
    pub log_size_bytes: u64,
    pub search_start_byte: u64,
    pub searched_bytes: u64,
    pub read_limit_bytes: u64,
    pub limited: bool,
}

impl AuditLogSearchScope {
    fn missing(path: PathBuf) -> Self {
        Self {
            path,
            log_size_bytes: 0,
            search_start_byte: 0,
            searched_bytes: 0,
            read_limit_bytes: AUDIT_LOG_READ_MAX_BYTES,
            limited: false,
        }
    }

    fn searched_window(path: PathBuf, log_size_bytes: u64, search_start_byte: u64) -> Self {
        let searched_bytes = log_size_bytes.saturating_sub(search_start_byte);
        Self {
            path,
            log_size_bytes,
            search_start_byte,
            searched_bytes,
            read_limit_bytes: AUDIT_LOG_READ_MAX_BYTES,
            limited: search_start_byte > 0,
        }
    }

    fn empty_search(path: PathBuf, log_size_bytes: u64) -> Self {
        Self {
            path,
            log_size_bytes,
            search_start_byte: log_size_bytes,
            searched_bytes: 0,
            read_limit_bytes: AUDIT_LOG_READ_MAX_BYTES,
            limited: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditLogReadResult {
    pub events: Vec<AuditLogEventRecord>,
    pub search_scope: AuditLogSearchScope,
}

pub fn audit_log_dir(default_log_dir: &Path) -> PathBuf {
    env::var_os("PRODEX_AUDIT_LOG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_log_dir.to_path_buf())
}

pub fn audit_log_path(default_log_dir: &Path) -> PathBuf {
    audit_log_dir(default_log_dir).join(AUDIT_LOG_FILE_NAME)
}

pub fn append_audit_event(
    path: &Path,
    component: &str,
    action: &str,
    outcome: &str,
    details: Value,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let now = Local::now();
    let event = AuditEvent {
        recorded_at: now.to_rfc3339(),
        recorded_at_epoch: now.timestamp(),
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
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    writeln!(file, "{line}").with_context(|| format!("failed to append {}", path.display()))?;
    Ok(())
}

pub fn format_audit_logs_summary(path: &Path) -> String {
    format!(
        "{} ({})",
        path.display(),
        if path.exists() { "exists" } else { "missing" }
    )
}

pub fn audit_logs_json_value(default_log_dir: &Path) -> Value {
    let directory = audit_log_dir(default_log_dir);
    let path = directory.join(AUDIT_LOG_FILE_NAME);
    serde_json::json!({
        "directory": directory.display().to_string(),
        "path": path.display().to_string(),
        "exists": path.exists(),
    })
}

pub fn read_recent_audit_events(
    path: &Path,
    query: &AuditLogQuery,
) -> Result<Vec<AuditLogEventRecord>> {
    if query.tail == 0 && !query.has_filters() {
        return Ok(Vec::new());
    }
    Ok(read_recent_audit_events_with_scope(path, query)?.events)
}

pub fn read_recent_audit_events_with_scope(
    path: &Path,
    query: &AuditLogQuery,
) -> Result<AuditLogReadResult> {
    if !path.exists() {
        return Ok(AuditLogReadResult {
            events: Vec::new(),
            search_scope: AuditLogSearchScope::missing(path.to_path_buf()),
        });
    }

    let candidate_read = if query.has_filters() {
        read_recent_audit_lines(path, None)?
    } else {
        read_recent_audit_lines(path, Some(query.tail))?
    };
    let mut matches = Vec::new();
    for line in candidate_read.lines {
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
        matches = matches.split_off(keep_from);
    }

    Ok(AuditLogReadResult {
        events: matches,
        search_scope: candidate_read.search_scope,
    })
}

pub fn render_audit_events_human_with_scope(
    path: &Path,
    query: &AuditLogQuery,
    events: &[AuditLogEventRecord],
    search_scope: Option<&AuditLogSearchScope>,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("Audit log: {}\n", path.display()));
    output.push_str(&format!("Tail: {}\n", query.tail));
    if let Some(search_scope) = search_scope {
        output.push_str(&format!(
            "Search scope: {}\n",
            format_audit_search_scope(search_scope)
        ));
    }
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

pub fn format_audit_search_scope(scope: &AuditLogSearchScope) -> String {
    let search_end_byte = scope.search_start_byte.saturating_add(scope.searched_bytes);
    let mut rendered = format!(
        "searched {} of {} bytes (byte range {}..{})",
        scope.searched_bytes, scope.log_size_bytes, scope.search_start_byte, search_end_byte
    );
    if scope.limited {
        rendered.push_str(&format!(
            " limited to last {} bytes",
            scope.read_limit_bytes
        ));
    }
    rendered
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

pub fn render_audit_events_human(
    path: &Path,
    query: &AuditLogQuery,
    events: &[AuditLogEventRecord],
) -> String {
    render_audit_events_human_with_scope(path, query, events, None)
}

pub fn format_audit_query(query: &AuditLogQuery) -> String {
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

struct AuditLogLineRead {
    lines: Vec<String>,
    search_scope: AuditLogSearchScope,
}

fn read_recent_audit_lines(path: &Path, tail: Option<usize>) -> Result<AuditLogLineRead> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let log_size_bytes = file
        .metadata()
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len();
    if tail == Some(0) {
        return Ok(AuditLogLineRead {
            lines: Vec::new(),
            search_scope: AuditLogSearchScope::empty_search(path.to_path_buf(), log_size_bytes),
        });
    }

    let start = log_size_bytes.saturating_sub(AUDIT_LOG_READ_MAX_BYTES);
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
        lines = lines.split_off(keep_from);
    }
    Ok(AuditLogLineRead {
        lines,
        search_scope: AuditLogSearchScope::searched_window(
            path.to_path_buf(),
            log_size_bytes,
            start,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = env::var_os(key);
            unsafe { env::set_var(key, value) };
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(value) = self.previous.as_ref() {
                unsafe { env::set_var(self.key, value) };
            } else {
                unsafe { env::remove_var(self.key) };
            }
        }
    }

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
        let _guard = EnvGuard::set("PRODEX_AUDIT_LOG_DIR", &dir.display().to_string());
        let fallback = temp_dir("fallback");

        assert_eq!(audit_log_path(&fallback), dir.join(AUDIT_LOG_FILE_NAME));

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
                "not-json\n",
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
