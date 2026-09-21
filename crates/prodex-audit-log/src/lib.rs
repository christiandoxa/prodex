use anyhow::{Context, Result, bail};
use chrono::{Datelike, Local, TimeZone, Utc};
use redaction::redaction_redact_json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

mod checksum;
mod private_file;
use checksum::{checksum_json, constant_time_text_eq, validate_audit_event};
use private_file::{open_no_follow_read, open_private_append_locked};

pub const AUDIT_LOG_FILE_NAME: &str = "prodex-audit.log";
pub const AUDIT_LOG_READ_MAX_BYTES: u64 = 512 * 1024;
pub const USAGE_LEDGER_FILE_NAME: &str = "prodex-usage-ledger.jsonl";
const AUDIT_RECORD_VERSION: u8 = 1;
const USAGE_LEDGER_RECORD_VERSION: u8 = 1;
const USAGE_LEDGER_READ_MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct AuditEventPayload<'a> {
    recorded_at: &'a str,
    recorded_at_epoch: i64,
    pid: u32,
    component: &'a str,
    action: &'a str,
    outcome: &'a str,
    details: &'a Value,
}

#[derive(Debug, Serialize)]
struct AuditEvent<'a> {
    schema_version: u8,
    #[serde(flatten)]
    payload: AuditEventPayload<'a>,
    checksum: String,
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
    #[serde(default)]
    pub schema_version: u8,
    pub recorded_at: String,
    pub recorded_at_epoch: i64,
    pub pid: u32,
    pub component: String,
    pub action: String,
    pub outcome: String,
    pub details: Value,
    #[serde(default)]
    pub checksum: Option<String>,
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
    audit_log_dir_with_override(default_log_dir, env::var_os("PRODEX_AUDIT_LOG_DIR"))
}

fn audit_log_dir_with_override(
    default_log_dir: &Path,
    override_dir: Option<std::ffi::OsString>,
) -> PathBuf {
    override_dir
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_log_dir.to_path_buf())
}

pub fn audit_log_path(default_log_dir: &Path) -> PathBuf {
    audit_log_dir(default_log_dir).join(AUDIT_LOG_FILE_NAME)
}

pub fn usage_ledger_path(default_log_dir: &Path) -> PathBuf {
    audit_log_dir(default_log_dir).join(USAGE_LEDGER_FILE_NAME)
}

pub fn append_audit_event(
    path: &Path,
    component: &str,
    action: &str,
    outcome: &str,
    mut details: Value,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    redaction_redact_json(&mut details);
    let now = Local::now();
    let recorded_at = now.to_rfc3339();
    let payload = AuditEventPayload {
        recorded_at: &recorded_at,
        recorded_at_epoch: now.timestamp(),
        pid: std::process::id(),
        component,
        action,
        outcome,
        details: &details,
    };
    let checksum = checksum_json(&payload).context("failed to checksum audit event")?;
    let event = AuditEvent {
        schema_version: AUDIT_RECORD_VERSION,
        payload,
        checksum,
    };
    let line = serde_json::to_string(&event).context("failed to serialize audit event")?;
    let mut file = open_private_append_locked(path)?;
    file.write_all(line.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_data())
        .with_context(|| format!("failed to append {}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageLedgerMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_domain: Option<String>,
}

impl UsageLedgerMetadata {
    pub fn redacted(
        profile_name: Option<&str>,
        account_id: Option<&str>,
        email: Option<&str>,
    ) -> Self {
        Self {
            profile_name: profile_name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.chars().take(100).collect()),
            account_hint: redacted_account_hint(account_id),
            email_domain: email_domain(email),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageLedgerRow {
    pub recorded_at_epoch: i64,
    pub source: String,
    pub operation: String,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default)]
    pub metadata: UsageLedgerMetadata,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub cost_micros: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct UsageLedgerEnvelope {
    schema_version: u8,
    row: UsageLedgerRow,
    checksum: String,
}

impl UsageLedgerRow {
    pub fn normalized(mut self) -> Self {
        self.source = normalize_usage_token(&self.source, "unknown", 64);
        self.operation = normalize_usage_token(&self.operation, "unknown", 64);
        self.outcome = normalize_usage_token(&self.outcome, "unknown", 32);
        self.model = self
            .model
            .map(|value| normalize_usage_token(&value, "", 128))
            .filter(|value| !value.is_empty());
        self.request_id = self
            .request_id
            .map(|value| normalize_usage_token(&value, "", 128))
            .filter(|value| !value.is_empty());
        if self.total_tokens == 0 {
            self.total_tokens = self
                .input_tokens
                .saturating_add(self.output_tokens)
                .saturating_add(self.reasoning_tokens);
        }
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BudgetWindow {
    Hour,
    Day,
    Week,
    Month,
}

impl BudgetWindow {
    pub fn start_epoch(self, now_epoch: i64) -> i64 {
        match self {
            Self::Hour => floor_epoch(now_epoch, 60 * 60),
            Self::Day => floor_epoch(now_epoch, 24 * 60 * 60),
            Self::Week => floor_epoch(now_epoch, 7 * 24 * 60 * 60),
            Self::Month => Utc
                .timestamp_opt(now_epoch, 0)
                .single()
                .and_then(|now| {
                    Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                        .single()
                })
                .map(|start| start.timestamp())
                .unwrap_or_else(|| floor_epoch(now_epoch, 30 * 24 * 60 * 60)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetLimit {
    pub key: String,
    pub window: BudgetWindow,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost_micros: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageLedgerSummary {
    pub since_epoch: i64,
    pub until_epoch: i64,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub cost_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetEvaluation {
    pub key: String,
    pub window: BudgetWindow,
    pub allowed: bool,
    pub reasons: Vec<String>,
    pub summary: UsageLedgerSummary,
}

pub fn usage_ledger_row_to_json_line(row: &UsageLedgerRow) -> Result<String> {
    let row = row.clone().normalized();
    let checksum = checksum_json(&row).context("failed to checksum usage ledger row")?;
    let line = serde_json::to_string(&UsageLedgerEnvelope {
        schema_version: USAGE_LEDGER_RECORD_VERSION,
        row,
        checksum,
    })
    .context("failed to serialize usage ledger row")?;
    Ok(format!("{line}\n"))
}

pub fn parse_usage_ledger_line(line: &str) -> Option<UsageLedgerRow> {
    parse_usage_ledger_line_checked(line).ok().flatten()
}

fn parse_usage_ledger_line_checked(line: &str) -> Result<Option<UsageLedgerRow>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if let Ok(envelope) = serde_json::from_str::<UsageLedgerEnvelope>(trimmed) {
        if envelope.schema_version != USAGE_LEDGER_RECORD_VERSION {
            bail!("unsupported usage ledger record version");
        }
        let expected = checksum_json(&envelope.row).context("failed to checksum usage row")?;
        if !constant_time_text_eq(&expected, &envelope.checksum) {
            bail!("usage ledger checksum mismatch");
        }
        return Ok(Some(envelope.row.normalized()));
    }
    let row =
        serde_json::from_str::<UsageLedgerRow>(trimmed).context("invalid usage ledger record")?;
    Ok(Some(row.normalized()))
}

pub fn append_usage_ledger_row(path: &Path, row: &UsageLedgerRow) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let line = usage_ledger_row_to_json_line(row)?;
    let mut file = open_private_append_locked(path)?;
    file.write_all(line.as_bytes())
        .and_then(|()| file.sync_data())
        .with_context(|| format!("failed to append {}", path.display()))?;
    Ok(())
}

pub fn read_usage_ledger_rows(path: &Path) -> Result<Vec<UsageLedgerRow>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = open_no_follow_read(path)?;
    let size = file
        .metadata()
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len();
    if size > USAGE_LEDGER_READ_MAX_BYTES {
        bail!("usage ledger exceeds bounded read limit");
    }
    let content = read_utf8_bounded(file, USAGE_LEDGER_READ_MAX_BYTES, "usage ledger")
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if let Some(row) = parse_usage_ledger_line_checked(line)
            .with_context(|| format!("invalid usage ledger record at line {}", index + 1))?
        {
            rows.push(row);
        }
    }
    Ok(rows)
}

pub fn summarize_usage_for_window(
    rows: &[UsageLedgerRow],
    window: BudgetWindow,
    now_epoch: i64,
) -> UsageLedgerSummary {
    let since_epoch = window.start_epoch(now_epoch);
    let mut summary = UsageLedgerSummary {
        since_epoch,
        until_epoch: now_epoch,
        ..UsageLedgerSummary::default()
    };
    for row in rows {
        if row.recorded_at_epoch < since_epoch || row.recorded_at_epoch > now_epoch {
            continue;
        }
        summary.requests = summary.requests.saturating_add(1);
        summary.input_tokens = summary.input_tokens.saturating_add(row.input_tokens);
        summary.output_tokens = summary.output_tokens.saturating_add(row.output_tokens);
        summary.cached_input_tokens = summary
            .cached_input_tokens
            .saturating_add(row.cached_input_tokens);
        summary.reasoning_tokens = summary
            .reasoning_tokens
            .saturating_add(row.reasoning_tokens);
        summary.total_tokens = summary.total_tokens.saturating_add(row.total_tokens);
        summary.cost_micros = summary.cost_micros.saturating_add(row.cost_micros);
    }
    summary
}

pub fn evaluate_budget_limit(
    limit: &BudgetLimit,
    rows: &[UsageLedgerRow],
    now_epoch: i64,
) -> BudgetEvaluation {
    let summary = summarize_usage_for_window(rows, limit.window, now_epoch);
    let mut reasons = Vec::new();
    if let Some(max_requests) = limit.max_requests
        && summary.requests >= max_requests
    {
        reasons.push(format!(
            "request limit reached ({}/{})",
            summary.requests, max_requests
        ));
    }
    if let Some(max_tokens) = limit.max_tokens
        && summary.total_tokens >= max_tokens
    {
        reasons.push(format!(
            "token limit reached ({}/{})",
            summary.total_tokens, max_tokens
        ));
    }
    if let Some(max_cost_micros) = limit.max_cost_micros
        && summary.cost_micros >= max_cost_micros
    {
        reasons.push(format!(
            "cost limit reached ({}/{})",
            summary.cost_micros, max_cost_micros
        ));
    }
    BudgetEvaluation {
        key: normalize_usage_token(&limit.key, "global", 100),
        window: limit.window,
        allowed: reasons.is_empty(),
        reasons,
        summary,
    }
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
    for (index, line) in candidate_read.lines.into_iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<AuditLogEventRecord>(&line)
            .with_context(|| format!("invalid audit log record at candidate line {}", index + 1))?;
        validate_audit_event(&event)
            .with_context(|| format!("invalid audit log record at candidate line {}", index + 1))?;
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

fn normalize_usage_token(value: &str, fallback: &str, max_chars: usize) -> String {
    let normalized = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/') {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(max_chars)
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized
    }
}

fn redacted_account_hint(account_id: Option<&str>) -> Option<String> {
    let account_id = account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let suffix = account_id
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    Some(format!("...{suffix}"))
}

fn email_domain(email: Option<&str>) -> Option<String> {
    let email = email.map(str::trim).filter(|value| !value.is_empty())?;
    let (_, domain) = email.rsplit_once('@')?;
    let domain = domain.trim().to_ascii_lowercase();
    (!domain.is_empty()).then_some(domain.chars().take(100).collect())
}

fn floor_epoch(value: i64, unit: i64) -> i64 {
    if unit <= 0 {
        return value;
    }
    value.div_euclid(unit).saturating_mul(unit)
}

struct AuditLogLineRead {
    lines: Vec<String>,
    search_scope: AuditLogSearchScope,
}

fn read_recent_audit_lines(path: &Path, tail: Option<usize>) -> Result<AuditLogLineRead> {
    let file = open_no_follow_read(path)?;
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
    let content = read_utf8_bounded(&mut reader, AUDIT_LOG_READ_MAX_BYTES, "audit log")
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

fn read_utf8_bounded(reader: impl Read, max_bytes: u64, label: &str) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {label}"))?;
    if bytes.len() as u64 > max_bytes {
        bail!("{label} exceeds bounded read limit");
    }
    String::from_utf8(bytes).with_context(|| format!("{label} is not valid UTF-8"))
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
