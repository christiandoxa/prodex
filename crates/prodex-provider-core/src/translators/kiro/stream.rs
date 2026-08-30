//! Kiro provider stream compatibility helpers.

use serde_json::{Value, json};

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation};

#[cfg(feature = "mojo")]
pub(super) fn kiro_mojo_body(input: KiroKernelInput<'_>) -> Vec<u8> {
    prodex_mojo_core::rich::kiro_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Kiro kernel failed: {error:?}"))
}

#[cfg(feature = "mojo")]
pub(super) fn kiro_mojo_value(input: KiroKernelInput<'_>) -> Value {
    let body = kiro_mojo_body(input);
    serde_json::from_slice(&body)
        .unwrap_or_else(|error| panic!("Mojo Kiro kernel returned invalid JSON: {error}"))
}

pub const KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS: usize = 128;
pub const KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_ID_BYTES: usize = 256;
const KIRO_PROVIDER_CORE_ACTIVITY_NAME_MAX_BYTES: usize = 160;
const KIRO_PROVIDER_CORE_ACTIVITY_KIND_MAX_BYTES: usize = 48;

pub fn kiro_provider_core_chat_completion_chunk(
    chat_completion_id: &str,
    model: Option<&str>,
    delta: Value,
    finish_reason: Option<&str>,
) -> Result<Vec<u8>, serde_json::Error> {
    #[cfg(feature = "mojo")]
    {
        let delta = serde_json::to_string(&delta)?;
        let mut input = KiroKernelInput::new(KiroKernelOperation::ChatCompletionChunk);
        input.response_id = Some(chat_completion_id);
        input.model = model;
        input.content = Some(&delta);
        input.finish_reason = finish_reason;
        return Ok(kiro_mojo_body(input));
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut chunk = json!({
            "id": chat_completion_id,
            "object": "chat.completion.chunk",
            "choices": [{
                "index": 0,
                "delta": delta,
            }],
        });
        if let Some(model) = model.filter(|value| !value.is_empty()) {
            chunk["model"] = Value::String(model.to_string());
        }
        if let Some(finish_reason) = finish_reason {
            chunk["choices"][0]["finish_reason"] = Value::String(finish_reason.to_string());
        }
        Ok(format!("data: {}\n\n", serde_json::to_string(&chunk)?).into_bytes())
    }
}

pub fn kiro_provider_core_chat_completion_role_delta() -> Value {
    #[cfg(feature = "mojo")]
    {
        return kiro_mojo_value(KiroKernelInput::new(KiroKernelOperation::ChatRoleDelta));
    }
    #[cfg(not(feature = "mojo"))]
    {
        json!({"role": "assistant"})
    }
}

pub fn kiro_provider_core_chat_completion_empty_delta() -> Value {
    #[cfg(feature = "mojo")]
    {
        return kiro_mojo_value(KiroKernelInput::new(KiroKernelOperation::ChatEmptyDelta));
    }
    #[cfg(not(feature = "mojo"))]
    {
        json!({})
    }
}

pub fn kiro_provider_core_chat_completion_text_delta(text: &str, include_role: bool) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::ChatTextDelta);
        input.content = Some(text);
        input.include_role = include_role;
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        if include_role {
            json!({"role": "assistant", "content": text})
        } else {
            json!({"content": text})
        }
    }
}

pub fn kiro_provider_core_chat_completion_reasoning_delta(text: &str, include_role: bool) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::ChatReasoningDelta);
        input.content = Some(text);
        input.include_role = include_role;
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        if include_role {
            json!({"role": "assistant", "reasoning_content": text})
        } else {
            json!({"reasoning_content": text})
        }
    }
}

pub fn kiro_provider_core_chat_completion_tool_call_delta(
    tool_call_id: &str,
    name: &str,
    arguments: &str,
    include_role: bool,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::ChatToolCallDelta);
        input.call_id = Some(tool_call_id);
        input.name = Some(name);
        input.arguments = Some(arguments);
        input.include_role = include_role;
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        let tool_call = json!({
            "index": 0,
            "id": tool_call_id,
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments,
            }
        });
        if include_role {
            json!({"role": "assistant", "tool_calls": [tool_call]})
        } else {
            json!({"tool_calls": [tool_call]})
        }
    }
}

pub fn kiro_provider_core_output_text_delta_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    delta: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::OutputTextDeltaEvent);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        input.content = Some(delta);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        json!({
            "type": "response.output_text.delta",
            "sequence_number": sequence_number,
            "created_at": created_at,
            "response_id": response_id,
            "delta": delta,
        })
    }
}

pub fn kiro_provider_core_response_created_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseCreatedEvent);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        json!({
            "type": "response.created",
            "sequence_number": sequence_number,
            "created_at": created_at,
            "response": {"id": response_id},
        })
    }
}

pub fn kiro_provider_core_output_item_added_event(sequence_number: u64, item: &Value) -> Value {
    #[cfg(feature = "mojo")]
    {
        let item = serde_json::to_string(item).expect("Kiro output item serializes");
        let mut input = KiroKernelInput::new(KiroKernelOperation::OutputItemAddedEvent);
        input.sequence_number = sequence_number;
        input.output = Some(&item);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        json!({
            "type": "response.output_item.added",
            "sequence_number": sequence_number,
            "item": item,
        })
    }
}

pub fn kiro_provider_core_output_item_done_event(
    sequence_number: u64,
    response_id: &str,
    item: &Value,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let item = serde_json::to_string(item).expect("Kiro output item serializes");
        let mut input = KiroKernelInput::new(KiroKernelOperation::OutputItemDoneEvent);
        input.sequence_number = sequence_number;
        input.response_id = Some(response_id);
        input.output = Some(&item);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        json!({
            "type": "response.output_item.done",
            "sequence_number": sequence_number,
            "item": item,
            "response_id": response_id,
        })
    }
}

pub fn kiro_provider_core_response_completed_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let response = serde_json::to_string(response).expect("Kiro response serializes");
        let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseCompletedEvent);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.output = Some(&response);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        kiro_provider_core_response_terminal_event(
            "response.completed",
            sequence_number,
            created_at,
            response,
        )
    }
}

pub fn kiro_provider_core_response_failed_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let response = serde_json::to_string(response).expect("Kiro response serializes");
        let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseFailedEvent);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.output = Some(&response);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        kiro_provider_core_response_terminal_event(
            "response.failed",
            sequence_number,
            created_at,
            response,
        )
    }
}

pub fn kiro_provider_core_response_incomplete_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let response = serde_json::to_string(response).expect("Kiro response serializes");
        let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseIncompleteEvent);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.output = Some(&response);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        kiro_provider_core_response_terminal_event(
            "response.incomplete",
            sequence_number,
            created_at,
            response,
        )
    }
}

#[cfg(not(feature = "mojo"))]
fn kiro_provider_core_response_terminal_event(
    event_type: &str,
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    json!({
        "type": event_type,
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response": response,
    })
}

pub fn kiro_provider_core_tool_call_arguments_delta_chat_value(
    tool_call_id: &str,
    arguments: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::ToolCallArgumentsDeltaChatValue);
        input.call_id = Some(tool_call_id);
        input.arguments = Some(arguments);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": tool_call_id,
                        "function": {
                            "arguments": arguments,
                        }
                    }]
                }
            }]
        })
    }
}

pub fn kiro_provider_core_stream_content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.is_empty()).then(|| text.clone()),
        Value::Array(items) => {
            let mut text = String::new();
            for item in items {
                if let Some(chunk) = kiro_provider_core_stream_content_text(item) {
                    text.push_str(&chunk);
                }
            }
            (!text.is_empty()).then_some(text)
        }
        Value::Object(object) => {
            if let Some(text) = object.get("text").and_then(Value::as_str) {
                return (!text.is_empty()).then(|| text.to_string());
            }
            object
                .get("content")
                .and_then(kiro_provider_core_stream_content_text)
        }
        _ => None,
    }
}

pub fn kiro_provider_core_stream_tool_call_item(
    _tool_call_id: &str,
    title: Option<&str>,
    status: Option<&str>,
    kind: Option<&str>,
    raw_input: Option<&Value>,
) -> Value {
    kiro_provider_core_tool_activity_item(title, status, kind, true, raw_input.is_some())
}

#[allow(clippy::too_many_arguments)]
pub fn kiro_provider_core_acp_responses_tool_call_item(
    _tool_call_id: &str,
    title: Option<&str>,
    status: Option<&str>,
    kind: Option<&str>,
    raw_input: Option<&Value>,
    raw_output: Option<&Value>,
    content: Option<&[Value]>,
    locations: Option<&[Value]>,
) -> Value {
    kiro_provider_core_tool_activity_item(
        title,
        status,
        kind,
        false,
        raw_input.is_some()
            || raw_output.is_some()
            || content.is_some_and(|items| !items.is_empty())
            || locations.is_some_and(|items| !items.is_empty()),
    )
}

pub fn kiro_provider_core_acp_chat_tool_call_item(
    _tool_call_id: &str,
    title: Option<&str>,
    kind: Option<&str>,
    raw_input: Option<&Value>,
) -> Value {
    kiro_provider_core_tool_activity_item(title, None, kind, false, raw_input.is_some())
}

pub fn kiro_provider_core_tool_activity_item(
    title: Option<&str>,
    status: Option<&str>,
    kind: Option<&str>,
    initial: bool,
    details_omitted: bool,
) -> Value {
    let safe_kind = kind.and_then(|value| {
        kiro_provider_core_safe_activity_field(value, KIRO_PROVIDER_CORE_ACTIVITY_KIND_MAX_BYTES)
    });
    let name = title
        .and_then(|value| {
            kiro_provider_core_safe_activity_field(
                value,
                KIRO_PROVIDER_CORE_ACTIVITY_NAME_MAX_BYTES,
            )
        })
        .or_else(|| safe_kind.clone())
        .unwrap_or_else(|| "Kiro internal activity".to_string());
    let status = kiro_provider_core_activity_status(status);
    let phase = match status.as_str() {
        "completed" => "completed",
        "failed" | "error" => "failed",
        "cancelled" => "cancelled",
        "truncated" => "truncated",
        _ if initial => "started",
        _ => "updated",
    };
    json!({
        "type": "kiro_internal_activity",
        "name": name,
        "status": status,
        "phase": phase,
        "kind": safe_kind,
        "details_omitted": details_omitted,
    })
}

pub fn kiro_provider_core_truncated_tool_activity_item() -> Value {
    kiro_provider_core_tool_activity_item(
        Some("Additional Kiro activities omitted"),
        Some("truncated"),
        None,
        false,
        true,
    )
}

pub fn kiro_provider_core_tool_activity_text(activity: &Value) -> String {
    let name = activity
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Kiro internal activity");
    let status = activity
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let phase = activity
        .get("phase")
        .and_then(Value::as_str)
        .unwrap_or("updated");
    let kind = activity
        .get("kind")
        .and_then(Value::as_str)
        .map(|kind| format!("; kind={kind}"))
        .unwrap_or_default();
    let details_omitted = activity
        .get("details_omitted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let details = if details_omitted {
        "; details=omitted"
    } else {
        ""
    };
    format!("[Kiro activity: {name}; status={status}; phase={phase}{kind}{details}]\n")
}

pub fn kiro_provider_core_acp_usage_update_json(
    used: u64,
    size: u64,
    cost: Option<(f64, &str)>,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let extra = serde_json::to_string(&json!({
            "cost": cost.map(|(amount, currency)| json!({
                "amount": amount,
                "currency": currency,
            })),
        }))
        .expect("Kiro ACP usage cost serializes");
        let mut input = KiroKernelInput::new(KiroKernelOperation::UsageUpdate);
        input.used = used;
        input.size = size;
        input.extra = Some(&extra);
        return kiro_mojo_value(input);
    }
    #[cfg(not(feature = "mojo"))]
    {
        json!({
            "used": used,
            "size": size,
            "remaining": size.saturating_sub(used),
            "cost": cost.map(|(amount, currency)| json!({
                "amount": amount,
                "currency": currency,
            })),
        })
    }
}

pub fn kiro_provider_core_stream_tool_arguments(raw_input: Option<&Value>) -> String {
    #[cfg(feature = "mojo")]
    {
        let mut input = KiroKernelInput::new(KiroKernelOperation::StreamToolArguments);
        input.input = raw_input.map(|_| "");
        return String::from_utf8(kiro_mojo_body(input))
            .expect("Mojo Kiro stream tool arguments are UTF-8");
    }
    #[cfg(not(feature = "mojo"))]
    {
        if raw_input.is_some() {
            r#"{"details_omitted":true}"#.to_string()
        } else {
            "{}".to_string()
        }
    }
}

fn kiro_provider_core_activity_status(status: Option<&str>) -> String {
    let normalized = status.unwrap_or_default().trim().to_ascii_lowercase();
    match normalized.as_str() {
        "pending" | "in_progress" | "running" | "completed" | "failed" | "error" | "cancelled"
        | "truncated" => normalized,
        _ => "unknown".to_string(),
    }
}

fn kiro_provider_core_safe_activity_field(value: &str, max_bytes: usize) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return None;
    }
    let lowercase = normalized.to_ascii_lowercase();
    if normalized.contains(['/', '\\', '@'])
        || [
            "authorization",
            "bearer",
            "api_key",
            "apikey",
            "password",
            "sk-",
            "sk_",
            "secret",
            "token",
            "credential",
        ]
        .iter()
        .any(|needle| lowercase.contains(needle))
    {
        return None;
    }
    let mut bounded = String::new();
    for ch in normalized.chars() {
        if bounded.len() + ch.len_utf8() > max_bytes {
            break;
        }
        bounded.push(ch);
    }
    (!bounded.is_empty()).then_some(bounded)
}
