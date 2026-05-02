use chrono::{Local, TimeZone};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SessionReport {
    pub id: String,
    pub thread_name: Option<String>,
    pub updated_at: Option<String>,
    pub cwd: Option<String>,
    pub profile: Option<String>,
    pub path: String,
    #[serde(skip)]
    updated_sort_key: i64,
    #[serde(skip)]
    cwd_path: Option<PathBuf>,
}

impl SessionReport {
    pub fn from_path(path: &Path, modified_epoch: i64) -> Self {
        Self {
            id: session_id_from_path(path),
            thread_name: None,
            updated_at: Some(format_epoch(modified_epoch)),
            cwd: None,
            profile: None,
            path: path.display().to_string(),
            updated_sort_key: modified_epoch,
            cwd_path: None,
        }
    }

    pub fn set_profile(&mut self, profile: Option<String>) {
        self.profile = profile;
    }

    pub fn matches_current_dir(&self, current_dir: &Path) -> bool {
        self.cwd_path.as_ref().is_some_and(|cwd| {
            normalize_path_for_compare(cwd) == normalize_path_for_compare(current_dir)
        })
    }
}

pub fn sort_session_reports(reports: &mut [SessionReport]) {
    reports.sort_by(|left, right| {
        right
            .updated_sort_key
            .cmp(&left.updated_sort_key)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.path.cmp(&right.path))
    });
}

pub fn is_session_metadata_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "jsonl" | "json"))
}

pub fn apply_session_json_lines<'a>(
    report: &mut SessionReport,
    lines: impl IntoIterator<Item = &'a str>,
) {
    for line in lines {
        apply_session_json_line(report, line);
    }
}

pub fn apply_session_json_line(report: &mut SessionReport, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        apply_session_value(report, &value);
    }
}

pub fn apply_session_value(report: &mut SessionReport, value: &serde_json::Value) {
    if let Some(id) = first_string_value(
        value,
        &[
            &["payload", "id"],
            &["payload", "session_id"],
            &["id"],
            &["session_id"],
        ],
    ) {
        report.id = id;
    }

    if let Some(thread_name) = first_string_value(
        value,
        &[
            &["payload", "thread_name"],
            &["payload", "title"],
            &["payload", "metadata", "thread_name"],
            &["thread_name"],
            &["title"],
            &["metadata", "thread_name"],
        ],
    ) {
        report.thread_name = Some(thread_name);
    }

    if let Some(cwd) = first_string_value(
        value,
        &[
            &["payload", "cwd"],
            &["payload", "metadata", "cwd"],
            &["payload", "workdir"],
            &["cwd"],
            &["metadata", "cwd"],
            &["workdir"],
        ],
    ) {
        report.cwd_path = Some(PathBuf::from(&cwd));
        report.cwd = Some(cwd);
    }

    if let Some(updated_at) = first_string_value(
        value,
        &[
            &["updated_at"],
            &["timestamp"],
            &["payload", "updated_at"],
            &["payload", "timestamp"],
        ],
    ) {
        report.updated_sort_key =
            timestamp_label_sort_key(&updated_at).unwrap_or(report.updated_sort_key);
        report.updated_at = Some(updated_at);
    } else if let Some(epoch) = first_i64_value(
        value,
        &[
            &["updated_at"],
            &["ts"],
            &["timestamp"],
            &["payload", "updated_at"],
            &["payload", "ts"],
            &["payload", "timestamp"],
        ],
    ) {
        report.updated_sort_key = epoch;
        report.updated_at = Some(format_epoch(epoch));
    }
}

pub fn first_string_value(value: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| value_at_path(value, path).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn first_i64_value(value: &serde_json::Value, paths: &[&[&str]]) -> Option<i64> {
    paths
        .iter()
        .find_map(|path| value_at_path(value, path).and_then(serde_json::Value::as_i64))
}

pub fn value_at_path<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub fn session_id_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-session")
        .to_string()
}

pub fn timestamp_label_sort_key(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.timestamp())
        .ok()
        .or_else(|| value.parse::<i64>().ok())
}

pub fn format_epoch(epoch: i64) -> String {
    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M:%S %Z").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_metadata_from_jsonl_values() {
        let mut report = SessionReport::from_path(Path::new("/tmp/session-a.jsonl"), 0);
        apply_session_json_line(
            &mut report,
            r#"{"timestamp":"2026-04-29T12:00:00Z","type":"session_meta","payload":{"id":"sess-a","thread_name":"Issue triage","cwd":"/tmp/workspace"}}"#,
        );

        assert_eq!(report.id, "sess-a");
        assert_eq!(report.thread_name.as_deref(), Some("Issue triage"));
        assert_eq!(report.cwd.as_deref(), Some("/tmp/workspace"));
        assert_eq!(report.updated_at.as_deref(), Some("2026-04-29T12:00:00Z"));
    }
}
