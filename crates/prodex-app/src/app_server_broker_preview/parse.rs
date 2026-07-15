use super::super::app_server_broker_protocol::app_server_broker_diagnostic_summary_json;
use serde_json::Value;

pub(crate) const APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES: usize = 1024 * 1024;

pub(crate) fn app_server_broker_preview_line(line: &str) -> Value {
    if line.len() > APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES {
        return serde_json::json!({
            "parse_ok": false,
            "error": "line_too_large",
            "message": format!(
                "app-server broker preview line exceeds {} bytes",
                APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES
            ),
        });
    }

    match serde_json::from_str::<Value>(line) {
        Ok(value) => serde_json::json!({
            "parse_ok": true,
            "summary": app_server_broker_diagnostic_summary_json(&value),
        }),
        Err(error) => serde_json::json!({
            "parse_ok": false,
            "error": "invalid_json",
            "message": error.to_string(),
        }),
    }
}
