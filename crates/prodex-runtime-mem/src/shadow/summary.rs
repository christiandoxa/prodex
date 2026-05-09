use super::*;

pub(super) fn runtime_mem_summary_has_critical_signal(summary: &str) -> bool {
    let lower = summary.to_ascii_lowercase();
    [
        "error",
        "failed",
        "failure",
        "panic",
        "exception",
        "traceback",
        "fatal",
        "cannot",
        "denied",
        "timed out",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(super) fn runtime_mem_shadow_summary_for_path(
    event: &Value,
    path: &str,
    label: &str,
    omitted_name: &str,
) -> Option<String> {
    let text = runtime_mem_lookup_json_path(event, path)?.as_str()?;
    let artifact_ref = runtime_mem_extract_artifact_marker(event);
    Some(runtime_mem_shadow_summary_from_text(
        text,
        label,
        omitted_name,
        artifact_ref.as_deref(),
    ))
}

pub(super) fn runtime_mem_shadow_summary_for_paths(
    event: &Value,
    paths: &[&str],
    label: &str,
    omitted_name: &str,
) -> Option<String> {
    let (_, text) = runtime_mem_first_text_path_at_paths(event, paths)?;
    let artifact_ref = runtime_mem_extract_artifact_marker(event);
    Some(runtime_mem_shadow_summary_from_text(
        &text,
        label,
        omitted_name,
        artifact_ref.as_deref(),
    ))
}

pub(super) fn runtime_mem_shadow_summary_from_text(
    text: &str,
    label: &str,
    omitted_name: &str,
    artifact_ref: Option<&str>,
) -> String {
    let label = runtime_mem_shadow_summary_label(label);
    let prefix_limit = runtime_mem_shadow_summary_prefix_char_limit(artifact_ref);
    let first_line = runtime_mem_first_useful_line(text)
        .map(|line| runtime_mem_truncate_chars(line, prefix_limit))
        .unwrap_or_else(|| "(empty)".to_string());
    let ref_part = artifact_ref
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" ref={value}"))
        .unwrap_or_default();
    format!(
        "{label}: {first_line} [b={} t~={} omit={omitted_name}{ref_part}]",
        text.len(),
        runtime_mem_approx_token_count(text),
    )
}

pub(super) fn runtime_mem_shadow_summary_label(label: &str) -> &str {
    match label {
        "user prompt" => "u",
        "assistant response" => "a",
        "tool output" => "tool",
        _ => label,
    }
}

pub(super) fn runtime_mem_shadow_summary_prefix_char_limit(artifact_ref: Option<&str>) -> usize {
    if artifact_ref.is_some_and(|value| runtime_mem_normalize_prodex_artifact_ref(value).is_some())
    {
        RUNTIME_MEM_SUPER_SLIM_REFERENCED_SUMMARY_PREFIX_CHAR_LIMIT
    } else {
        RUNTIME_MEM_SUPER_SLIM_SUMMARY_PREFIX_CHAR_LIMIT
    }
}

pub(super) fn runtime_mem_value_contains_text(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.contains(needle),
        Value::Array(values) => values
            .iter()
            .any(|value| runtime_mem_value_contains_text(value, needle)),
        Value::Object(object) => object
            .values()
            .any(|value| runtime_mem_value_contains_text(value, needle)),
        _ => false,
    }
}
