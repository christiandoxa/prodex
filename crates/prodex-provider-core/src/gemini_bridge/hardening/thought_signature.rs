//! Gemini tool-call thought-signature hardening.

const GEMINI_SYNTHETIC_THOUGHT_SIGNATURE: &str = "skip_thought_signature_validator";

pub fn gemini_provider_core_harden_tool_call_thought_signatures(
    body: &mut Vec<u8>,
    model: &str,
) -> Result<usize, serde_json::Error> {
    if !gemini_provider_core_model_requires_tool_thought_signature(model) {
        return Ok(0);
    }
    let mut value: serde_json::Value = serde_json::from_slice(body)?;
    let injected = if let Some(contents) = value
        .get_mut("request")
        .and_then(|request| request.get_mut("contents"))
        .and_then(serde_json::Value::as_array_mut)
    {
        gemini_provider_core_harden_contents_tool_call_thought_signatures(contents)
    } else if let Some(contents) = value
        .get_mut("contents")
        .and_then(serde_json::Value::as_array_mut)
    {
        gemini_provider_core_harden_contents_tool_call_thought_signatures(contents)
    } else {
        0
    };
    if injected > 0 {
        *body = serde_json::to_vec(&value)?;
    }
    Ok(injected)
}

pub fn gemini_provider_core_thought_signature(part: &serde_json::Value) -> Option<String> {
    part.get("thoughtSignature")
        .or_else(|| part.get("thought_signature"))
        .and_then(serde_json::Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .map(str::to_string)
}

fn gemini_provider_core_model_requires_tool_thought_signature(model: &str) -> bool {
    model.contains("gemini-3")
}

fn gemini_provider_core_harden_contents_tool_call_thought_signatures(
    contents: &mut [serde_json::Value],
) -> usize {
    let mut injected = 0;
    for content in contents {
        if content.get("role").and_then(serde_json::Value::as_str) != Some("model") {
            continue;
        }
        let Some(parts) = content
            .get_mut("parts")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        let mut found_function_call = false;
        for part in parts {
            if part.get("functionCall").is_none() {
                continue;
            }
            if !found_function_call
                && gemini_provider_core_thought_signature(part).is_none()
                && let Some(object) = part.as_object_mut()
            {
                object.insert(
                    "thoughtSignature".to_string(),
                    serde_json::Value::String(GEMINI_SYNTHETIC_THOUGHT_SIGNATURE.to_string()),
                );
                injected += 1;
            }
            found_function_call = true;
        }
    }
    injected
}
