//! Kiro provider stream compatibility helpers.

use serde_json::{Value, json};

use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation};

pub(super) fn kiro_mojo_body(input: KiroKernelInput<'_>) -> Vec<u8> {
    prodex_mojo_core::rich::kiro_kernel(input)
        .unwrap_or_else(|error| panic!("Mojo Kiro kernel failed: {error:?}"))
}

pub(super) fn kiro_mojo_value(input: KiroKernelInput<'_>) -> Value {
    let body = kiro_mojo_body(input);
    serde_json::from_slice(&body)
        .unwrap_or_else(|error| panic!("Mojo Kiro kernel returned invalid JSON: {error}"))
}

pub const KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_EVENTS: usize = 128;
pub const KIRO_PROVIDER_CORE_MAX_TOOL_ACTIVITY_ID_BYTES: usize = 256;

pub fn kiro_provider_core_chat_completion_chunk(
    chat_completion_id: &str,
    model: Option<&str>,
    delta: Value,
    finish_reason: Option<&str>,
) -> Result<Vec<u8>, serde_json::Error> {
    let delta = serde_json::to_string(&delta)?;
    let mut input = KiroKernelInput::new(KiroKernelOperation::ChatCompletionChunk);
    input.response_id = Some(chat_completion_id);
    input.model = model;
    input.content = Some(&delta);
    input.finish_reason = finish_reason;
    Ok(kiro_mojo_body(input))
}

pub fn kiro_provider_core_chat_completion_role_delta() -> Value {
    kiro_mojo_value(KiroKernelInput::new(KiroKernelOperation::ChatRoleDelta))
}

pub fn kiro_provider_core_chat_completion_empty_delta() -> Value {
    kiro_mojo_value(KiroKernelInput::new(KiroKernelOperation::ChatEmptyDelta))
}

pub fn kiro_provider_core_chat_completion_text_delta(text: &str, include_role: bool) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::ChatTextDelta);
    input.content = Some(text);
    input.include_role = include_role;
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_chat_completion_reasoning_delta(text: &str, include_role: bool) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::ChatReasoningDelta);
    input.content = Some(text);
    input.include_role = include_role;
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_chat_completion_tool_call_delta(
    tool_call_id: &str,
    name: &str,
    arguments: &str,
    include_role: bool,
) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::ChatToolCallDelta);
    input.call_id = Some(tool_call_id);
    input.name = Some(name);
    input.arguments = Some(arguments);
    input.include_role = include_role;
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_output_text_delta_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    delta: &str,
) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::OutputTextDeltaEvent);
    input.sequence_number = sequence_number;
    input.created_at = created_at;
    input.response_id = Some(response_id);
    input.content = Some(delta);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_response_created_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseCreatedEvent);
    input.sequence_number = sequence_number;
    input.created_at = created_at;
    input.response_id = Some(response_id);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_output_item_added_event(sequence_number: u64, item: &Value) -> Value {
    let item = serde_json::to_string(item).expect("Kiro output item serializes");
    let mut input = KiroKernelInput::new(KiroKernelOperation::OutputItemAddedEvent);
    input.sequence_number = sequence_number;
    input.output = Some(&item);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_output_item_done_event(
    sequence_number: u64,
    response_id: &str,
    item: &Value,
) -> Value {
    let item = serde_json::to_string(item).expect("Kiro output item serializes");
    let mut input = KiroKernelInput::new(KiroKernelOperation::OutputItemDoneEvent);
    input.sequence_number = sequence_number;
    input.response_id = Some(response_id);
    input.output = Some(&item);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_response_completed_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    let response = serde_json::to_string(response).expect("Kiro response serializes");
    let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseCompletedEvent);
    input.sequence_number = sequence_number;
    input.created_at = created_at;
    input.output = Some(&response);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_response_failed_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    let response = serde_json::to_string(response).expect("Kiro response serializes");
    let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseFailedEvent);
    input.sequence_number = sequence_number;
    input.created_at = created_at;
    input.output = Some(&response);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_response_incomplete_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    let response = serde_json::to_string(response).expect("Kiro response serializes");
    let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseIncompleteEvent);
    input.sequence_number = sequence_number;
    input.created_at = created_at;
    input.output = Some(&response);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_tool_call_arguments_delta_chat_value(
    tool_call_id: &str,
    arguments: &str,
) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::ToolCallArgumentsDeltaChatValue);
    input.call_id = Some(tool_call_id);
    input.arguments = Some(arguments);
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_stream_content_text(value: &Value) -> Option<String> {
    let serialized = serde_json::to_string(value).expect("Kiro stream content serializes");
    let mut input = KiroKernelInput::new(KiroKernelOperation::StreamContentText);
    input.input = Some(&serialized);
    let text = String::from_utf8(kiro_mojo_body(input)).expect("Mojo Kiro stream content is UTF-8");
    (!text.is_empty()).then_some(text)
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
    let mut input = KiroKernelInput::new(KiroKernelOperation::ToolActivityItem);
    input.name = title;
    input.model = kind;
    input.status = status;
    input.include_role = initial;
    input.has_tool_calls = details_omitted;
    kiro_mojo_value(input)
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
    let mut input = KiroKernelInput::new(KiroKernelOperation::ToolActivityText);
    input.name = activity.get("name").and_then(Value::as_str);
    input.status = activity.get("status").and_then(Value::as_str);
    input.role = activity.get("phase").and_then(Value::as_str);
    input.model = activity.get("kind").and_then(Value::as_str);
    input.has_tool_calls = activity
        .get("details_omitted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    String::from_utf8(kiro_mojo_body(input)).expect("Mojo Kiro activity text is UTF-8")
}

pub fn kiro_provider_core_acp_usage_update_json(
    used: u64,
    size: u64,
    cost: Option<(f64, &str)>,
) -> Value {
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
    kiro_mojo_value(input)
}

pub fn kiro_provider_core_stream_tool_arguments(raw_input: Option<&Value>) -> String {
    let mut input = KiroKernelInput::new(KiroKernelOperation::StreamToolArguments);
    input.input = raw_input.map(|_| "");
    String::from_utf8(kiro_mojo_body(input)).expect("Mojo Kiro stream tool arguments are UTF-8")
}
