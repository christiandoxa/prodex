//! Gemini content sequence hardening.

mod parts;
mod tool_pairs;

use self::parts::gemini_provider_core_content_role;
use self::tool_pairs::{
    gemini_provider_core_pair_tool_calls, gemini_provider_core_refine_tool_response_order,
    gemini_provider_core_repair_orphan_tool_responses,
};

pub fn gemini_provider_core_harden_contents(contents: &mut Vec<serde_json::Value>) {
    gemini_provider_core_coalesce_contents(contents);
    gemini_provider_core_pair_tool_calls(contents);
    gemini_provider_core_repair_orphan_tool_responses(contents);
    gemini_provider_core_refine_tool_response_order(contents);
    gemini_provider_core_enforce_content_role_shape(contents);
}

fn gemini_provider_core_coalesce_contents(contents: &mut Vec<serde_json::Value>) {
    let mut coalesced = Vec::new();
    for mut content in contents.drain(..) {
        let role = gemini_provider_core_content_role(&content)
            .unwrap_or("user")
            .to_string();
        let Some(parts) = content
            .get_mut("parts")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        if parts.is_empty() {
            continue;
        }
        content["role"] = serde_json::Value::String(role.clone());
        if let Some(last) = coalesced.last_mut()
            && gemini_provider_core_content_role(last) == Some(role.as_str())
        {
            if let (Some(last_parts), Some(next_parts)) = (
                last.get_mut("parts")
                    .and_then(serde_json::Value::as_array_mut),
                content
                    .get_mut("parts")
                    .and_then(serde_json::Value::as_array_mut),
            ) {
                last_parts.append(next_parts);
            }
            continue;
        }
        coalesced.push(content);
    }
    *contents = coalesced;
}

fn gemini_provider_core_enforce_content_role_shape(contents: &mut Vec<serde_json::Value>) {
    if contents.is_empty() {
        return;
    }
    if gemini_provider_core_content_role(&contents[0]) == Some("model") {
        contents.insert(
            0,
            serde_json::json!({
                "role": "user",
                "parts": [{ "text": "[Continuing from previous Codex context.]" }],
            }),
        );
    }
    if contents.last().and_then(gemini_provider_core_content_role) == Some("model") {
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{ "text": "Please continue." }],
        }));
    }
    gemini_provider_core_coalesce_contents(contents);
}
