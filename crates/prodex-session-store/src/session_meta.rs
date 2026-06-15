use chrono::{SecondsFormat, Utc};
use prodex_app_reports::first_string_value;
use std::path::Path;

pub(crate) fn line_starts_codex_rollout_metadata(line: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .is_some_and(|value| value_starts_codex_rollout_metadata(&value))
}

fn value_starts_codex_rollout_metadata(value: &serde_json::Value) -> bool {
    if value
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .is_none()
    {
        return false;
    }
    if value.get("type").and_then(serde_json::Value::as_str) != Some("session_meta") {
        return false;
    }
    let Some(payload) = value.get("payload").and_then(serde_json::Value::as_object) else {
        return false;
    };
    [
        "id",
        "timestamp",
        "cwd",
        "originator",
        "cli_version",
        "source",
    ]
    .iter()
    .all(|field| {
        payload
            .get(*field)
            .and_then(serde_json::Value::as_str)
            .is_some()
    })
}

pub(crate) fn synthetic_session_metadata_line(
    path: &Path,
    selector: &str,
    lines: &[String],
) -> Option<String> {
    let session_id = lines
        .iter()
        .find_map(|line| super::session_line_resume_id_matching_mode(line, selector, false))
        .or_else(|| super::session_path_id_matching_selector(path, selector, false))
        .or_else(|| super::full_codex_session_id(selector).map(ToOwned::to_owned))?;
    let timestamp = synthetic_session_timestamp(lines);
    let cwd = synthetic_session_cwd(lines);
    let model_provider = lines.iter().find_map(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|value| {
                first_string_value(
                    &value,
                    &[&["payload", "model_provider"], &["model_provider"]],
                )
            })
    });
    let mut payload = serde_json::Map::new();
    payload.insert("id".to_string(), serde_json::Value::String(session_id));
    payload.insert(
        "timestamp".to_string(),
        serde_json::Value::String(timestamp.clone()),
    );
    payload.insert("cwd".to_string(), serde_json::Value::String(cwd));
    payload.insert(
        "originator".to_string(),
        serde_json::Value::String("prodex-repair".to_string()),
    );
    payload.insert(
        "cli_version".to_string(),
        serde_json::Value::String(env!("CARGO_PKG_VERSION").to_string()),
    );
    payload.insert(
        "source".to_string(),
        serde_json::Value::String("cli".to_string()),
    );
    if let Some(model_provider) = model_provider {
        payload.insert(
            "model_provider".to_string(),
            serde_json::Value::String(model_provider),
        );
    }

    Some(
        serde_json::json!({
            "timestamp": timestamp,
            "type": "session_meta",
            "payload": payload,
        })
        .to_string(),
    )
}

fn synthetic_session_timestamp(lines: &[String]) -> String {
    lines
        .iter()
        .find_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|value| {
                    first_string_value(&value, &[&["timestamp"], &["payload", "timestamp"]])
                })
        })
        .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn synthetic_session_cwd(lines: &[String]) -> String {
    lines
        .iter()
        .find_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|value| first_string_value(&value, &[&["payload", "cwd"], &["cwd"]]))
        })
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| ".".to_string())
}
