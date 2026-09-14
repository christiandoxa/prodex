const RUNTIME_JSON_SCAN_LIMIT: usize = 2_048;

pub(super) fn runtime_json_find<T, F>(root: &serde_json::Value, mut candidate: F) -> Option<T>
where
    F: FnMut(&serde_json::Value) -> Option<T>,
{
    let mut stack = vec![root];
    let mut visited = 0usize;
    while let Some(value) = stack.pop() {
        if let Some(result) = candidate(value) {
            return Some(result);
        }
        visited += 1;
        if visited >= RUNTIME_JSON_SCAN_LIMIT {
            break;
        }
        match value {
            serde_json::Value::Array(values) => stack.extend(values.iter().rev()),
            serde_json::Value::Object(map) => stack.extend(map.values().rev()),
            _ => {}
        }
    }
    None
}
