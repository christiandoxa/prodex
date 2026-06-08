pub(super) fn runtime_gemini_harden_contents(contents: &mut Vec<serde_json::Value>) {
    runtime_gemini_coalesce_contents(contents);
    runtime_gemini_pair_tool_calls(contents);
    runtime_gemini_repair_orphan_tool_responses(contents);
    runtime_gemini_refine_tool_response_order(contents);
    runtime_gemini_enforce_content_role_shape(contents);
}

fn runtime_gemini_coalesce_contents(contents: &mut Vec<serde_json::Value>) {
    let mut coalesced = Vec::new();
    for mut content in contents.drain(..) {
        let role = runtime_gemini_content_role(&content)
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
            && runtime_gemini_content_role(last) == Some(role.as_str())
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

fn runtime_gemini_pair_tool_calls(contents: &mut Vec<serde_json::Value>) {
    let mut index = 0;
    while index < contents.len() {
        if runtime_gemini_content_role(&contents[index]) != Some("model") {
            index += 1;
            continue;
        }
        let calls = runtime_gemini_function_calls_in_content(&contents[index]);
        if calls.is_empty() {
            index += 1;
            continue;
        }
        let existing = contents
            .get(index + 1)
            .filter(|content| runtime_gemini_content_role(content) == Some("user"))
            .map(runtime_gemini_function_responses_in_content)
            .unwrap_or_default();
        let missing = calls
            .into_iter()
            .filter(|call| !existing.iter().any(|response| response == call))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            index += 1;
            continue;
        }
        let sentinel_parts = missing
            .into_iter()
            .map(|(id, name)| {
                serde_json::json!({
                    "functionResponse": {
                        "id": id,
                        "name": name,
                        "response": {
                            "error": "The tool execution result was missing from the Codex conversation history. Treat this tool result as unavailable and recover by inspecting current workspace state before continuing."
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        if let Some(next) = contents.get_mut(index + 1)
            && runtime_gemini_content_role(next) == Some("user")
            && let Some(parts) = next
                .get_mut("parts")
                .and_then(serde_json::Value::as_array_mut)
        {
            parts.extend(sentinel_parts);
        } else {
            contents.insert(
                index + 1,
                serde_json::json!({
                    "role": "user",
                    "parts": sentinel_parts,
                }),
            );
        }
        index += 2;
    }
}

fn runtime_gemini_repair_orphan_tool_responses(contents: &mut Vec<serde_json::Value>) {
    let mut index = 0;
    while index < contents.len() {
        if runtime_gemini_content_role(&contents[index]) != Some("user") {
            index += 1;
            continue;
        }
        let responses = runtime_gemini_function_responses_in_content(&contents[index]);
        if responses.is_empty() {
            index += 1;
            continue;
        }
        let previous_calls = index
            .checked_sub(1)
            .and_then(|previous| contents.get(previous))
            .filter(|content| runtime_gemini_content_role(content) == Some("model"))
            .map(runtime_gemini_function_calls_in_content)
            .unwrap_or_default();
        let orphaned = responses
            .into_iter()
            .filter(|response| !previous_calls.iter().any(|call| call == response))
            .collect::<Vec<_>>();
        if orphaned.is_empty() {
            index += 1;
            continue;
        }
        let synthetic_parts = orphaned
            .into_iter()
            .map(|(id, name)| {
                serde_json::json!({
                    "functionCall": {
                        "id": id,
                        "name": name,
                        "args": {}
                    },
                    "thoughtSignature": "skip_thought_signature_validator"
                })
            })
            .collect::<Vec<_>>();
        if index > 0 && runtime_gemini_content_role(&contents[index - 1]) == Some("model") {
            if let Some(parts) = contents[index - 1]
                .get_mut("parts")
                .and_then(serde_json::Value::as_array_mut)
            {
                parts.extend(synthetic_parts);
            }
        } else {
            contents.insert(
                index,
                serde_json::json!({
                    "role": "model",
                    "parts": synthetic_parts,
                }),
            );
            index += 1;
        }
        index += 1;
    }
}

fn runtime_gemini_refine_tool_response_order(contents: &mut [serde_json::Value]) {
    for index in 1..contents.len() {
        if runtime_gemini_content_role(&contents[index]) != Some("user")
            || runtime_gemini_content_role(&contents[index - 1]) != Some("model")
        {
            continue;
        }
        let order = runtime_gemini_function_calls_in_content(&contents[index - 1])
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if order.is_empty() {
            continue;
        }
        let Some(parts) = contents[index]
            .get_mut("parts")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        let mut response_parts = Vec::new();
        let mut other_parts = Vec::new();
        for part in parts.drain(..) {
            if part.get("functionResponse").is_some() {
                response_parts.push(part);
            } else {
                other_parts.push(part);
            }
        }
        response_parts.sort_by_key(|part| {
            let id = part
                .get("functionResponse")
                .and_then(|response| response.get("id"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            order
                .iter()
                .position(|candidate| candidate == id)
                .unwrap_or(usize::MAX)
        });
        parts.extend(response_parts);
        parts.extend(other_parts);
    }
}

fn runtime_gemini_enforce_content_role_shape(contents: &mut Vec<serde_json::Value>) {
    if contents.is_empty() {
        return;
    }
    if runtime_gemini_content_role(&contents[0]) == Some("model") {
        contents.insert(
            0,
            serde_json::json!({
                "role": "user",
                "parts": [{ "text": "[Continuing from previous Codex context.]" }],
            }),
        );
    }
    if contents.last().and_then(runtime_gemini_content_role) == Some("model") {
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{ "text": "Please continue." }],
        }));
    }
    runtime_gemini_coalesce_contents(contents);
}

fn runtime_gemini_content_role(content: &serde_json::Value) -> Option<&str> {
    content.get("role").and_then(serde_json::Value::as_str)
}

fn runtime_gemini_function_calls_in_content(content: &serde_json::Value) -> Vec<(String, String)> {
    runtime_gemini_parts_in_content(content)
        .into_iter()
        .filter_map(|part| {
            let function_call = part.get("functionCall")?;
            Some((
                function_call
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                function_call
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
            ))
        })
        .collect()
}

fn runtime_gemini_function_responses_in_content(
    content: &serde_json::Value,
) -> Vec<(String, String)> {
    runtime_gemini_parts_in_content(content)
        .into_iter()
        .filter_map(|part| {
            let function_response = part.get("functionResponse")?;
            Some((
                function_response
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                function_response
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
            ))
        })
        .collect()
}

fn runtime_gemini_parts_in_content(content: &serde_json::Value) -> Vec<&serde_json::Value> {
    content
        .get("parts")
        .and_then(serde_json::Value::as_array)
        .map(|parts| parts.iter().collect())
        .unwrap_or_default()
}
