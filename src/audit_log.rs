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

#[derive(Debug, Clone, Serialize)]
pub(super) struct AuditLogSearchScope {
    pub(super) path: PathBuf,
    pub(super) log_size_bytes: u64,
    pub(super) search_start_byte: u64,
    pub(super) searched_bytes: u64,
    pub(super) read_limit_bytes: u64,
    pub(super) limited: bool,
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
pub(super) struct AuditLogReadResult {
    pub(super) events: Vec<AuditLogEventRecord>,
    pub(super) search_scope: AuditLogSearchScope,
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

#[cfg(test)]
pub(super) fn read_recent_audit_events(query: &AuditLogQuery) -> Result<Vec<AuditLogEventRecord>> {
    if query.tail == 0 && !query.has_filters() {
        return Ok(Vec::new());
    }
    Ok(read_recent_audit_events_with_scope(query)?.events)
}

pub(super) fn read_recent_audit_events_with_scope(
    query: &AuditLogQuery,
) -> Result<AuditLogReadResult> {
    let path = audit_log_path();
    if !path.exists() {
        return Ok(AuditLogReadResult {
            events: Vec::new(),
            search_scope: AuditLogSearchScope::missing(path),
        });
    }

    let candidate_read = if query.has_filters() {
        read_recent_audit_lines(&path, None)?
    } else {
        read_recent_audit_lines(&path, Some(query.tail))?
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

pub(super) fn render_audit_events_human_with_scope(
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

pub(super) fn format_audit_search_scope(scope: &AuditLogSearchScope) -> String {
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

#[cfg(test)]
pub(super) fn render_audit_events_human(
    path: &Path,
    query: &AuditLogQuery,
    events: &[AuditLogEventRecord],
) -> String {
    render_audit_events_human_with_scope(path, query, events, None)
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
mod tests;
