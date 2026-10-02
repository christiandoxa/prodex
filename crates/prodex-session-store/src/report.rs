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

fn session_json_node_kind_and_text(
    value: &serde_json::Value,
) -> (prodex_mojo_core::json::JsonKind, &str) {
    use prodex_mojo_core::json::JsonKind;
    match value {
        serde_json::Value::Null => (JsonKind::Null, ""),
        serde_json::Value::Bool(false) => (JsonKind::False, ""),
        serde_json::Value::Bool(true) => (JsonKind::True, ""),
        serde_json::Value::Number(_) => (JsonKind::Number, ""),
        serde_json::Value::String(value) => (JsonKind::String, value.as_str()),
        serde_json::Value::Array(_) => (JsonKind::Array, ""),
        serde_json::Value::Object(_) => (JsonKind::Object, ""),
    }
}

fn session_link_json_child(
    nodes: &mut [prodex_mojo_core::json::JsonNode<'_>],
    parent: usize,
    previous: Option<usize>,
    child: usize,
) {
    if let Some(previous) = previous {
        nodes[previous].next_sibling = Some(child);
    } else {
        nodes[parent].first_child = Some(child);
    }
}

fn session_push_json_node<'a>(
    nodes: &mut Vec<prodex_mojo_core::json::JsonNode<'a>>,
    number_values: Option<&mut Vec<Option<i64>>>,
    value: &'a serde_json::Value,
    key: &'a str,
    parent: Option<usize>,
) -> usize {
    use prodex_mojo_core::json::JsonNode;

    let (kind, text) = session_json_node_kind_and_text(value);
    let index = nodes.len();
    nodes.push(JsonNode {
        kind,
        first_child: None,
        next_sibling: None,
        parent,
        key,
        text,
        raw_start: 0,
        raw_length: 0,
    });
    let mut number_values = number_values;
    if let Some(values) = number_values.as_deref_mut() {
        values.push(value.as_i64());
    }

    let mut previous = None;
    match value {
        serde_json::Value::Array(values) => {
            for child in values {
                let child_index = session_push_json_node(
                    nodes,
                    number_values.as_deref_mut(),
                    child,
                    "",
                    Some(index),
                );
                session_link_json_child(nodes, index, previous, child_index);
                previous = Some(child_index);
            }
        }
        serde_json::Value::Object(map) => {
            for (child_key, child) in map {
                let child_index = session_push_json_node(
                    nodes,
                    number_values.as_deref_mut(),
                    child,
                    child_key.as_str(),
                    Some(index),
                );
                session_link_json_child(nodes, index, previous, child_index);
                previous = Some(child_index);
            }
        }
        _ => {}
    }
    index
}

fn session_json_nodes<'a>(
    value: &'a serde_json::Value,
) -> Vec<prodex_mojo_core::json::JsonNode<'a>> {
    let mut nodes = Vec::new();
    session_push_json_node(&mut nodes, None, value, "", None);
    nodes
}

fn session_json_nodes_with_numbers<'a>(
    value: &'a serde_json::Value,
) -> (Vec<prodex_mojo_core::json::JsonNode<'a>>, Vec<Option<i64>>) {
    let mut nodes = Vec::new();
    let mut number_values = Vec::new();
    session_push_json_node(&mut nodes, Some(&mut number_values), value, "", None);
    (nodes, number_values)
}

pub(super) fn session_value_metadata(value: &serde_json::Value) -> SessionValueMetadata {
    let nodes = session_json_nodes(value);
    let plan = prodex_mojo_core::json::session_report_metadata(&nodes)
        .expect("Mojo session-report metadata planner returned invalid output");
    session_value_metadata_from_plan(&nodes, plan)
}

fn session_value_metadata_from_plan(
    nodes: &[prodex_mojo_core::json::JsonNode<'_>],
    plan: prodex_mojo_core::json::SessionReportMetadataPlan,
) -> SessionValueMetadata {
    let string_at = |index: Option<usize>| index.map(|index| nodes[index].text.trim().to_string());
    SessionValueMetadata {
        type_class: plan.type_class,
        resume_id: string_at(plan.resume_id),
        model: string_at(plan.model),
        effort: string_at(plan.effort),
        thread_name: string_at(plan.thread_name),
        cwd: string_at(plan.cwd),
        updated_at: string_at(plan.updated_at),
        parent_thread_id: string_at(plan.parent_thread_id),
        model_provider: string_at(plan.model_provider),
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
    let (nodes, number_values) = session_json_nodes_with_numbers(value);
    let plan = prodex_mojo_core::json::session_report_update_plan(&nodes, &number_values)
        .expect("Mojo session-report update planner returned invalid output");
    let metadata = session_value_metadata_from_plan(&nodes, plan.metadata);

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
        if let Some(updated_sort_key) = plan.updated_sort_key {
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
