use chrono::{SecondsFormat, Utc};
use std::path::Path;

pub(crate) fn line_starts_codex_rollout_metadata(line: &str) -> bool {
    prodex_mojo_core::json::session_report_update_json(line)
        .ok()
        .is_some_and(|plan| plan.starts_rollout_metadata)
}

fn session_meta_span_string(raw: &str, span: Option<(usize, usize)>) -> Option<String> {
    span.map(|(start, end)| {
        serde_json::from_str::<String>(&raw[start..end])
            .expect("Mojo session-meta plan selected a JSON string token")
            .trim()
            .to_string()
    })
}

#[derive(Default)]
struct SyntheticSessionFields {
    timestamp: Option<String>,
    cwd: Option<String>,
    model_provider: Option<String>,
}

fn synthetic_session_fields(lines: &[String]) -> SyntheticSessionFields {
    let mut fields = SyntheticSessionFields::default();
    for line in lines {
        let Ok(plan) = prodex_mojo_core::json::session_report_update_json(line) else {
            continue;
        };
        if fields.timestamp.is_none() {
            fields.timestamp = session_meta_span_string(line, plan.repair_timestamp);
        }
        if fields.cwd.is_none() {
            fields.cwd = session_meta_span_string(line, plan.repair_cwd);
        }
        if fields.model_provider.is_none() {
            fields.model_provider = session_meta_span_string(line, plan.repair_model_provider);
        }
        if fields.timestamp.is_some() && fields.cwd.is_some() && fields.model_provider.is_some() {
            break;
        }
    }
    fields
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
    let repair = synthetic_session_fields(lines);
    let timestamp = repair
        .timestamp
        .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true));
    let cwd = repair
        .cwd
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| ".".to_string());
    let model_provider = repair.model_provider;
    let mut payload = serde_json::Map::new();
    payload.insert(
        "session_id".to_string(),
        serde_json::Value::String(session_id.clone()),
    );
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
