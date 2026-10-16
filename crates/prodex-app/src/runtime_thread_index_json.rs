#[derive(serde::Deserialize)]
struct DirtyMarkerBoundary {
    schema_version: serde_json::Value,
    rollout_path: serde_json::Value,
}

pub(super) fn dirty_marker_value(contents: &[u8]) -> Option<serde_json::Value> {
    let text = std::str::from_utf8(contents).ok()?;
    let boundary = serde_json::from_str::<DirtyMarkerBoundary>(text).ok()?;
    Some(serde_json::Value::Object(serde_json::Map::from_iter([
        ("schema_version".to_string(), boundary.schema_version),
        ("rollout_path".to_string(), boundary.rollout_path),
    ])))
}
