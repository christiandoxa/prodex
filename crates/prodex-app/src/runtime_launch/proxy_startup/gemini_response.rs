use super::super::deepseek_rewrite::{
    runtime_deepseek_created_at, runtime_deepseek_rtk_wrapped_tool_arguments,
};
use super::super::gemini_thought_signatures::runtime_gemini_thought_signature;
use super::super::provider_tools::runtime_provider_split_flat_namespace_tool_name;
use prodex_cli::SUPER_GEMINI_DEFAULT_MODEL;
use std::borrow::Cow;

const GEMINI_CUSTOM_APPLY_PATCH_TOOL: &str = "apply_patch";

pub(in super::super) fn runtime_gemini_normalized_response_value(
    value: &serde_json::Value,
) -> Cow<'_, serde_json::Value> {
    let Some(response) = value.get("response") else {
        return Cow::Borrowed(value);
    };
    let Some(trace_id) = value.get("traceId").and_then(serde_json::Value::as_str) else {
        return Cow::Borrowed(response);
    };
    let mut response = response.clone();
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "responseId".to_string(),
            serde_json::Value::String(trace_id.to_string()),
        );
    }
    Cow::Owned(response)
}

pub(in super::super) fn runtime_gemini_responses_usage(
    usage: &serde_json::Value,
) -> Option<serde_json::Value> {
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("candidatesTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    Some(serde_json::json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens,
    }))
}

pub(in super::super) fn runtime_gemini_chat_assistant_messages_from_generate_value(
    value: &serde_json::Value,
    request_id: u64,
) -> Vec<serde_json::Value> {
    let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    let mut text = String::new();
    let mut reasoning_content = String::new();
    let mut tool_calls = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if let Some(part_text) = part.get("text").and_then(serde_json::Value::as_str) {
            if part
                .get("thought")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                reasoning_content.push_str(part_text);
            } else {
                text.push_str(part_text);
            }
        }
        if let Some(function_call) = part.get("functionCall") {
            let name = function_call
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("tool_call");
            let call_id = runtime_gemini_function_call_id(function_call, request_id, index);
            let args = function_call
                .get("args")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let args = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_string());
            let mut tool_call = serde_json::json!({
                "id": call_id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": runtime_deepseek_rtk_wrapped_tool_arguments(name, &args),
                },
            });
            if let Some(signature) = runtime_gemini_thought_signature(part)
                .or_else(|| runtime_gemini_thought_signature(function_call))
            {
                tool_call["gemini_thought_signature"] = serde_json::Value::String(signature);
            }
            tool_calls.push(tool_call);
        }
    }
    if text.is_empty() && reasoning_content.is_empty() && tool_calls.is_empty() {
        return Vec::new();
    }
    let mut assistant = serde_json::json!({
        "role": "assistant",
        "content": if text.is_empty() {
            if tool_calls.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(String::new())
            }
        } else {
            serde_json::Value::String(text)
        },
    });
    if !reasoning_content.is_empty() {
        assistant["reasoning_content"] = serde_json::Value::String(reasoning_content);
    }
    if !tool_calls.is_empty() {
        assistant["tool_calls"] = serde_json::Value::Array(tool_calls);
    }
    vec![assistant]
}

pub(in super::super) fn runtime_gemini_responses_value_from_generate_value(
    value: &serde_json::Value,
    request_id: u64,
) -> serde_json::Value {
    let response_id = runtime_gemini_response_id(value, request_id);
    let model = runtime_gemini_model(value);
    let mut output = Vec::new();
    if let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    {
        let mut text = String::new();
        for (index, part) in parts.iter().enumerate() {
            if let Some(part_text) = part.get("text").and_then(serde_json::Value::as_str)
                && !part
                    .get("thought")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            {
                text.push_str(part_text);
            }
            if let Some(function_call) = part.get("functionCall") {
                output.push(runtime_gemini_responses_tool_call_item(
                    part,
                    function_call,
                    request_id,
                    index,
                ));
            }
        }
        if !text.is_empty() {
            output.insert(
                0,
                serde_json::json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": text,
                    }],
                }),
            );
        }
    }
    if let Some(grounding_call) = runtime_gemini_web_search_call_from_grounding(value, &response_id)
    {
        output.push(grounding_call);
    }
    let mut response = serde_json::json!({
        "id": response_id,
        "object": "response",
        "created_at": runtime_deepseek_created_at(),
        "model": model,
        "output": output,
    });
    if let Some(usage) = value
        .get("usageMetadata")
        .and_then(runtime_gemini_responses_usage)
    {
        response["usage"] = usage;
    }
    response
}

pub(in super::super) fn runtime_gemini_web_search_call_from_grounding(
    value: &serde_json::Value,
    response_id: &str,
) -> Option<serde_json::Value> {
    let candidate = value.get("candidates")?.as_array()?.first()?;
    let metadata = candidate.get("groundingMetadata")?;

    let mut sources = Vec::new();
    if let Some(chunks) = metadata.get("groundingChunks").and_then(|v| v.as_array()) {
        for chunk in chunks {
            if let Some(web) = chunk.get("web") {
                let Some(uri) = web.get("uri").and_then(|v| v.as_str()) else {
                    continue;
                };
                let mut source = serde_json::json!({
                    "type": "url",
                    "url": uri,
                });
                if let Some(title) = web.get("title").and_then(|v| v.as_str()) {
                    source["title"] = serde_json::Value::String(title.to_string());
                }
                sources.push(source);
            }
        }
    }

    let queries = metadata
        .get("webSearchQueries")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();

    if sources.is_empty() && queries.is_empty() {
        return None;
    }

    Some(serde_json::json!({
        "type": "web_search_call",
        "id": format!("ws_{response_id}"),
        "status": "completed",
        "action": {
            "type": "search",
            "queries": queries,
            "sources": sources,
        },
    }))
}

fn runtime_gemini_response_id(value: &serde_json::Value, request_id: u64) -> String {
    value
        .get("responseId")
        .or_else(|| value.get("id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("resp_gemini_{request_id}"))
}

fn runtime_gemini_model(value: &serde_json::Value) -> String {
    value
        .get("modelVersion")
        .or_else(|| value.get("model"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(SUPER_GEMINI_DEFAULT_MODEL)
        .to_string()
}

fn runtime_gemini_responses_tool_call_item(
    part: &serde_json::Value,
    function_call: &serde_json::Value,
    request_id: u64,
    index: usize,
) -> serde_json::Value {
    let call_id = runtime_gemini_function_call_id(function_call, request_id, index);
    let flat_name = function_call
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool_call");
    let args_value = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    if flat_name == "tool_search" {
        return serde_json::json!({
            "type": "tool_search_call",
            "call_id": call_id,
            "execution": "client",
            "arguments": args_value,
        });
    }
    if let Some(item) = runtime_gemini_custom_tool_call_item(&call_id, flat_name, &args_value) {
        return item;
    }
    let args = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());
    let (namespace, name) = runtime_provider_split_flat_namespace_tool_name(flat_name);
    let mut item = serde_json::json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": runtime_deepseek_rtk_wrapped_tool_arguments(flat_name, &args),
    });
    if let Some(namespace) = namespace {
        item["namespace"] = serde_json::Value::String(namespace);
    }
    if let Some(signature) = runtime_gemini_thought_signature(part)
        .or_else(|| runtime_gemini_thought_signature(function_call))
    {
        item["gemini_thought_signature"] = serde_json::Value::String(signature);
    }
    item
}

pub(in super::super) fn runtime_gemini_custom_tool_call_item(
    call_id: &str,
    flat_name: &str,
    args_value: &serde_json::Value,
) -> Option<serde_json::Value> {
    if flat_name != GEMINI_CUSTOM_APPLY_PATCH_TOOL {
        return None;
    }
    Some(serde_json::json!({
        "type": "custom_tool_call",
        "call_id": call_id,
        "name": flat_name,
        "input": runtime_gemini_custom_tool_input_from_args_value(args_value),
    }))
}

pub(in super::super) fn runtime_gemini_custom_tool_input_from_arguments(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .map(|value| runtime_gemini_custom_tool_input_from_args_value(&value))
        .unwrap_or_else(|_| arguments.to_string())
}

fn runtime_gemini_custom_tool_input_from_args_value(args_value: &serde_json::Value) -> String {
    match args_value {
        serde_json::Value::String(input) => input.clone(),
        serde_json::Value::Object(object) => ["input", "patch", "text", "content"]
            .iter()
            .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
            .map(str::to_string)
            .unwrap_or_else(|| args_value.to_string()),
        _ => args_value.to_string(),
    }
}

fn runtime_gemini_function_call_id(
    function_call: &serde_json::Value,
    request_id: u64,
    index: usize,
) -> String {
    function_call
        .get("id")
        .and_then(serde_json::Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("call_gemini_{request_id}_{index}"))
}
