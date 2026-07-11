//! Shared JSON and tool-name helpers for chat tool translation.

pub fn provider_core_flatten_namespace_tool_name(namespace: &str, name: &str) -> String {
    let namespace = namespace.trim();
    let name = name.trim();
    if namespace.starts_with("mcp__")
        && !namespace.ends_with('_')
        && !name.starts_with('_')
        && !name.contains("__")
    {
        format!("{namespace}__{name}")
    } else {
        format!("{namespace}--{name}")
    }
}

pub(super) fn provider_core_function_name_segment(value: &str) -> Option<String> {
    let segment = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    (!segment.is_empty()).then_some(segment)
}

pub(super) fn provider_core_json_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

pub(super) fn provider_core_json_string_at_path(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &[&str],
) -> Option<String> {
    let mut value = object.get(*path.first()?)?;
    for key in &path[1..] {
        value = value.get(*key)?;
    }
    value.as_str().map(str::to_string)
}
