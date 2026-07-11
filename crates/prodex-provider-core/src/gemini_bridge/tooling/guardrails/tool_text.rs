//! Tool-output text extraction for Gemini guardrails.

pub(super) fn gemini_provider_core_tool_text_has_version_lines(text: &str) -> bool {
    text.lines().any(|line| {
        let lower = line.trim().to_ascii_lowercase();
        let starts_with_tool = [
            "rtk ",
            "sqz ",
            "sqz-mcp ",
            "token-savior ",
            "claw-compactor ",
            "prodex ",
            "codex ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
        starts_with_tool && lower.chars().any(|ch| ch.is_ascii_digit())
    })
}

pub(super) fn gemini_provider_core_tool_texts_since_latest_user(
    messages: &[serde_json::Value],
) -> Vec<String> {
    let latest_user = messages
        .iter()
        .rposition(|message| {
            message.get("role").and_then(serde_json::Value::as_str) == Some("user")
        })
        .unwrap_or(0);
    messages
        .iter()
        .skip(latest_user + 1)
        .filter(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("tool"))
        .map(|message| {
            let mut text = String::new();
            gemini_provider_core_collect_payload_text(
                message.get("output").or_else(|| message.get("content")),
                &mut text,
            );
            text
        })
        .filter(|text| !text.trim().is_empty())
        .collect()
}

pub(super) fn gemini_provider_core_tool_text_has_failure(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "process exited with code 1",
        "process exited with code 2",
        "process exited with code 127",
        "no such file or directory",
        "command not found",
        "error:",
        "failed",
        "not found",
        "virtual manifest",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub(super) fn gemini_provider_core_collect_payload_text(
    value: Option<&serde_json::Value>,
    output: &mut String,
) {
    match value {
        None => {}
        Some(value) => match value {
            serde_json::Value::String(text) => {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(text);
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    gemini_provider_core_collect_payload_text(Some(value), output);
                }
            }
            serde_json::Value::Object(object) => {
                for key in ["output", "content", "text"] {
                    if let Some(value) = object.get(key) {
                        gemini_provider_core_collect_payload_text(Some(value), output);
                        break;
                    }
                }
            }
            _ => {}
        },
    }
}
