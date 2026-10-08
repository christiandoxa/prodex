//! Kiro ACP compatibility helpers.

use super::stream::kiro_mojo_body;
use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation};
use serde_json::Value;

fn kiro_acp_mojo_value(input: KiroKernelInput<'_>) -> Value {
    serde_json::from_slice(&kiro_mojo_body(input)).expect("Mojo Kiro ACP shape is valid JSON")
}

fn kiro_acp_mojo_optional_value(input: KiroKernelInput<'_>) -> Option<Value> {
    let body = kiro_mojo_body(input);
    (!body.is_empty())
        .then(|| serde_json::from_slice(&body).expect("Mojo Kiro optional ACP shape is valid JSON"))
}

pub fn kiro_provider_core_acp_initialize_request(
    id: u64,
    client_name: &str,
    client_title: &str,
    client_version: &str,
) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpInitializeRequest);
    input.request_id = id;
    input.name = Some(client_name);
    input.content = Some(client_title);
    input.model = Some(client_version);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_session_new_request(id: u64, cwd: &str) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpSessionNewRequest);
    input.request_id = id;
    input.content = Some(cwd);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_session_prompt_request(
    id: u64,
    session_id: &str,
    prompt: &str,
) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpSessionPromptRequest);
    input.request_id = id;
    input.response_id = Some(session_id);
    input.content = Some(prompt);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_model_value(model_id: &str, name: &str) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpModel);
    input.model = Some(model_id);
    input.name = Some(name);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_assistant_output_message(text: &str) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpAssistantOutput);
    input.content = Some(text);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_response_value(
    response_id: &str,
    created_at: u64,
    model: &str,
    output: Vec<Value>,
) -> Value {
    let output = serde_json::to_string(&output).expect("Kiro ACP output serializes");
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpResponse);
    input.response_id = Some(response_id);
    input.created_at = created_at;
    input.model = Some(model);
    input.output = Some(&output);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_chat_assistant_message(
    assistant_text: &str,
    reasoning_text: &str,
    tool_calls: Vec<Value>,
) -> Option<Value> {
    let tool_calls = serde_json::to_string(&tool_calls).expect("Kiro ACP tool calls serialize");
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpChatAssistant);
    input.content = Some(assistant_text);
    input.reason = Some(reasoning_text);
    input.tool_calls = Some(&tool_calls);
    kiro_acp_mojo_optional_value(input)
}

pub fn kiro_provider_core_acp_plan_entry(content: &str, priority: &str, status: &str) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpPlanEntry);
    input.content = Some(content);
    input.reason = Some(priority);
    input.status = Some(status);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_error_value(code: i64, message: &str) -> Value {
    let code = code.to_string();
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpError);
    input.call_id = Some(&code);
    input.content = Some(message);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_mark_failed_response(response: &mut Value, code: i64, message: &str) {
    response["status"] = Value::String("failed".to_string());
    response["error"] = kiro_provider_core_acp_error_value(code, message);
}

pub fn kiro_provider_core_acp_session_info(title: Option<&str>, updated_at: Option<&str>) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpSessionInfo);
    input.name = title;
    input.status = updated_at;
    kiro_acp_mojo_value(input)
}

#[allow(clippy::too_many_arguments)]
pub fn kiro_provider_core_acp_metadata(
    reasoning_text: &str,
    usage_update: Option<Value>,
    plan_entries: Option<Vec<Value>>,
    available_commands: Option<Vec<Value>>,
    current_mode_id: Option<&str>,
    session_title: Option<&str>,
    session_updated_at: Option<&str>,
    stop_reason: Option<&str>,
    tool_activities: Vec<Value>,
) -> Option<Value> {
    let usage_update = usage_update
        .as_ref()
        .map(|value| serde_json::to_string(value).expect("Kiro ACP usage serializes"));
    let plan_entries = plan_entries
        .as_ref()
        .map(|value| serde_json::to_string(value).expect("Kiro ACP plan serializes"));
    let available_commands = available_commands
        .as_ref()
        .map(|value| serde_json::to_string(value).expect("Kiro ACP commands serialize"));
    let tool_activities =
        serde_json::to_string(&tool_activities).expect("Kiro ACP tool activities serialize");
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpMetadata);
    input.reason = Some(reasoning_text);
    input.input = usage_update.as_deref();
    input.output = plan_entries.as_deref();
    input.tool_calls = available_commands.as_deref();
    input.model = current_mode_id;
    input.name = session_title;
    input.status = session_updated_at;
    input.finish_reason = stop_reason;
    input.extra = Some(&tool_activities);
    kiro_acp_mojo_optional_value(input)
}

pub fn kiro_provider_core_acp_stop_reason(result: Option<&Value>) -> Option<String> {
    let result =
        result.map(|result| serde_json::to_string(result).expect("Kiro ACP result serializes"));
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpStopReason);
    input.input = result.as_deref();
    serde_json::from_slice(&kiro_mojo_body(input))
        .expect("Mojo Kiro ACP stop reason is a JSON string or null")
}

pub fn kiro_provider_core_acp_incomplete_details(
    stop_reason: Option<&str>,
) -> Option<(&'static str, &'static str)> {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpIncompleteReason);
    input.status = stop_reason;
    let tag: u8 = serde_json::from_slice(&kiro_mojo_body(input))
        .expect("Mojo Kiro ACP incomplete reason is a JSON tag");
    match tag {
        0 => None,
        1 => Some((
            "max_output_tokens",
            "Kiro stopped before end_turn because the model hit its output limit.",
        )),
        2 => Some((
            "max_turn_requests",
            "Kiro stopped before end_turn because the turn hit its request limit.",
        )),
        3 => Some(("refusal", "Kiro refused to continue the turn.")),
        4 => Some(("cancelled", "Kiro cancelled the turn before completion.")),
        _ => panic!("Mojo Kiro ACP incomplete reason returned an invalid tag: {tag}"),
    }
}

pub fn kiro_provider_core_acp_incomplete_details_value(reason: &str, message: &str) -> Value {
    let mut input = KiroKernelInput::new(KiroKernelOperation::AcpIncompleteDetails);
    input.reason = Some(reason);
    input.content = Some(message);
    kiro_acp_mojo_value(input)
}

pub fn kiro_provider_core_acp_mark_incomplete_response(
    response: &mut Value,
    reason: &str,
    message: &str,
) {
    response["status"] = Value::String("incomplete".to_string());
    response["incomplete_details"] =
        kiro_provider_core_acp_incomplete_details_value(reason, message);
}
