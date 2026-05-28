pub(super) fn runtime_deepseek_json_string(
    object: &serde_json::Map<String, serde_json::Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

pub(super) fn runtime_deepseek_json_string_at_path(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &[&str],
) -> Option<String> {
    let mut value = object.get(*path.first()?)?;
    for key in path.iter().skip(1) {
        value = value.get(*key)?;
    }
    value.as_str().map(str::to_string)
}
