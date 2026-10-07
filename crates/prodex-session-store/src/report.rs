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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(skip)]
    last_model: Option<String>,
    #[serde(skip)]
    last_reasoning_effort: Option<String>,
    #[serde(skip)]
    updated_sort_key: i64,
    #[serde(skip)]
    cwd_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SessionValueMetadata {
    pub(super) type_class: i64,
    pub(super) resume_id: Option<String>,
    pub(super) model: Option<String>,
    pub(super) effort: Option<String>,
    pub(super) thread_name: Option<String>,
    pub(super) cwd: Option<String>,
    pub(super) updated_at: Option<String>,
    pub(super) parent_thread_id: Option<String>,
    pub(super) model_provider: Option<String>,
}

fn session_string_from_span(raw: &str, span: Option<(usize, usize)>) -> Option<String> {
    span.map(|(start, end)| {
        serde_json::from_str::<String>(&raw[start..end])
            .expect("Mojo session-report plan selected a JSON string token")
            .trim()
            .to_string()
    })
}

pub(super) fn session_value_metadata(value: &serde_json::Value) -> SessionValueMetadata {
    let raw = serde_json::to_string(value).expect("session report JSON serializes");
    let plan = prodex_mojo_core::json::session_report_metadata_json(&raw)
        .expect("Mojo session-report metadata planner returned invalid output");
    session_value_metadata_from_plan(&raw, plan)
}

fn session_value_metadata_from_plan(
    raw: &str,
    plan: prodex_mojo_core::json::SessionReportMetadataPlan,
) -> SessionValueMetadata {
    SessionValueMetadata {
        type_class: plan.type_class,
        resume_id: session_string_from_span(raw, plan.resume_id),
        model: session_string_from_span(raw, plan.model),
        effort: session_string_from_span(raw, plan.effort),
        thread_name: session_string_from_span(raw, plan.thread_name),
        cwd: session_string_from_span(raw, plan.cwd),
        updated_at: session_string_from_span(raw, plan.updated_at),
        parent_thread_id: session_string_from_span(raw, plan.parent_thread_id),
        model_provider: session_string_from_span(raw, plan.model_provider),
    }
}

impl SessionReport {
    pub fn from_path(path: &Path, modified_epoch: i64) -> Self {
        Self {
            id: session_id_from_path(path),
            thread_name: None,
            updated_at: Some(format_epoch(modified_epoch)),
            cwd: None,
            profile: None,
            model_provider: None,
            path: path.display().to_string(),
            parent_thread_id: None,
            last_model: None,
            last_reasoning_effort: None,
            updated_sort_key: modified_epoch,
            cwd_path: None,
        }
    }

    pub fn set_profile(&mut self, profile: Option<String>) {
        self.profile = profile;
    }

    pub fn set_model_provider(&mut self, model_provider: Option<String>) {
        self.model_provider = model_provider;
    }

    pub fn last_model(&self) -> Option<&str> {
        self.last_model.as_deref()
    }

    pub fn last_reasoning_effort(&self) -> Option<&str> {
        self.last_reasoning_effort.as_deref()
    }

    pub fn matches_current_dir(&self, current_dir: &Path) -> bool {
        self.cwd_path.as_ref().is_some_and(|cwd| {
            normalize_path_for_compare(cwd) == normalize_path_for_compare(current_dir)
        })
    }

    pub fn is_subagent(&self) -> bool {
        self.parent_thread_id.is_some()
    }
}

pub fn sort_session_reports(reports: &mut [SessionReport]) {
    let keys = reports
        .iter()
        .map(|report| prodex_mojo_core::json::SessionReportOrderKey {
            updated_sort_key: report.updated_sort_key,
            id: &report.id,
            path: &report.path,
        })
        .collect::<Vec<_>>();
    let order = prodex_mojo_core::json::session_report_order(&keys)
        .expect("Mojo session-report ordering returned invalid output");
    let sorted = order
        .into_iter()
        .map(|index| reports[index].clone())
        .collect::<Vec<_>>();
    reports.clone_from_slice(&sorted);
}

pub fn is_session_metadata_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.ends_with(".jsonl") || name.ends_with(".jsonl.zst") || name.ends_with(".json")
        })
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
    let raw = serde_json::to_string(value).expect("session report JSON serializes");
    let plan = prodex_mojo_core::json::session_report_update_json(&raw)
        .expect("Mojo session-report update planner returned invalid output");
    let metadata = session_value_metadata_from_plan(&raw, plan.metadata);
    let string_timestamp_sort_key = metadata
        .updated_at
        .as_deref()
        .and_then(timestamp_label_sort_key);

    if let Some(model) = metadata.model {
        report.last_model = Some(model);
    }
    if let Some(effort) = metadata.effort {
        report.last_reasoning_effort = Some(effort);
    }

    if plan.update_resume_id.is_some() {
        report.id = metadata
            .resume_id
            .expect("Mojo session-report update plan omitted eligible ID metadata");
    }

    if let Some(thread_name) = metadata.thread_name {
        report.thread_name = Some(thread_name);
    }

    if let Some(cwd) = metadata.cwd {
        report.cwd_path = Some(PathBuf::from(&cwd));
        report.cwd = Some(cwd);
    }

    if let Some(updated_at) = metadata.updated_at {
        if let Some(updated_sort_key) = string_timestamp_sort_key {
            report.updated_sort_key = updated_sort_key;
        }
        report.updated_at = Some(updated_at);
    } else if let Some(epoch) = plan.numeric_timestamp {
        report.updated_sort_key = epoch;
        report.updated_at = Some(format_epoch(epoch));
    }

    if let Some(parent_thread_id) = metadata.parent_thread_id {
        report.parent_thread_id = Some(parent_thread_id);
    }

    if let Some(model_provider) = metadata.model_provider {
        report.model_provider = Some(model_provider);
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
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| {
            name.strip_suffix(".jsonl.zst")
                .or_else(|| name.strip_suffix(".jsonl"))
                .or_else(|| name.strip_suffix(".json"))
                .or(Some(name))
        })
        .unwrap_or("unknown-session")
        .to_string()
}

pub fn timestamp_label_sort_key(value: &str) -> Option<i64> {
    prodex_mojo_core::json::session_report_timestamp_sort_key(value)
        .expect("Mojo session-report timestamp parser returned invalid output")
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
#[path = "../tests/src/report.rs"]
mod tests;
