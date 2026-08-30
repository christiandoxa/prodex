#[path = "openai_chat_compat_request/validation.rs"]
mod validation;

use super::{ProviderId, Value, stringify_arguments, value_to_text};
#[cfg(not(feature = "mojo"))]
use serde_json::json;

#[derive(Clone, Copy)]
enum RequestMessageKind {
    System,
    User,
    Role,
    FunctionCall,
    FunctionCallOutput,
}

fn request_message(
    kind: RequestMessageKind,
    role: Option<&str>,
    text: Option<&str>,
    call_id: Option<&str>,
    namespace: Option<&str>,
    name: Option<&str>,
    arguments: Option<&str>,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let kind = match kind {
            RequestMessageKind::System => prodex_mojo_core::rich::OpenAiCompatMessageKind::System,
            RequestMessageKind::User => prodex_mojo_core::rich::OpenAiCompatMessageKind::User,
            RequestMessageKind::Role => prodex_mojo_core::rich::OpenAiCompatMessageKind::Role,
            RequestMessageKind::FunctionCall => {
                prodex_mojo_core::rich::OpenAiCompatMessageKind::FunctionCall
            }
            RequestMessageKind::FunctionCallOutput => {
                prodex_mojo_core::rich::OpenAiCompatMessageKind::FunctionCallOutput
            }
        };
        let body = prodex_mojo_core::rich::openai_compat_request_message(
            prodex_mojo_core::rich::OpenAiCompatMessageInput {
                kind,
                role,
                text,
                call_id,
                namespace,
                name,
                arguments,
            },
        )
        .unwrap_or_else(|error| {
            panic!("Mojo OpenAI compatibility request message failed: {error:?}")
        });
        return serde_json::from_slice(&body).unwrap_or_else(|error| {
            panic!("Mojo OpenAI compatibility request message was invalid JSON: {error}")
        });
    }

    #[cfg(not(feature = "mojo"))]
    {
        match kind {
            RequestMessageKind::System => {
                json!({"role": "system", "content": text.unwrap_or_default()})
            }
            RequestMessageKind::User => {
                json!({"role": "user", "content": text.unwrap_or_default()})
            }
            RequestMessageKind::Role => {
                json!({"role": role.unwrap_or("user"), "content": text.unwrap_or_default()})
            }
            RequestMessageKind::FunctionCall => {
                let name = name.unwrap_or_default();
                let full_name = namespace
                    .filter(|namespace| !namespace.is_empty())
                    .map(|namespace| format!("{namespace}.{name}"))
                    .unwrap_or_else(|| name.to_string());
                json!({
                    "role": "assistant",
                    "content": "",
                    "tool_calls": [{
                        "id": call_id.unwrap_or("call_1"),
                        "type": "function",
                        "function": {
                            "name": full_name,
                            "arguments": arguments.unwrap_or("{}"),
                        }
                    }]
                })
            }
            RequestMessageKind::FunctionCallOutput => json!({
                "role": "tool",
                "tool_call_id": call_id.unwrap_or_default(),
                "content": text.unwrap_or_default(),
            }),
        }
    }
}

pub(super) fn responses_messages_to_chat_messages(
    obj: &serde_json::Map<String, Value>,
) -> Vec<Value> {
    let mut messages = Vec::new();
    if let Some(instructions) = obj.get("instructions").and_then(value_to_text)
        && !instructions.is_empty()
    {
        messages.push(request_message(
            RequestMessageKind::System,
            None,
            Some(&instructions),
            None,
            None,
            None,
            None,
        ));
    }

    if let Some(input) = obj.get("input") {
        messages.extend(messages_from_input_value(input));
    }

    messages
}

fn messages_from_input_value(value: &Value) -> Vec<Value> {
    if let Some(text) = value_to_text(value)
        && !text.is_empty()
    {
        return vec![request_message(
            RequestMessageKind::User,
            None,
            Some(&text),
            None,
            None,
            None,
            None,
        )];
    }

    match value {
        Value::Array(items) => items.iter().flat_map(messages_from_input_item).collect(),
        Value::Object(_) => messages_from_input_item(value),
        _ => Vec::new(),
    }
}

pub(super) fn validate_responses_chat_compat_request(
    provider: ProviderId,
    obj: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    validation::validate_responses_chat_compat_request(provider, obj)
}

fn messages_from_input_item(value: &Value) -> Vec<Value> {
    if let Some(text) = value_to_text(value)
        && value.is_string()
    {
        return vec![request_message(
            RequestMessageKind::User,
            None,
            Some(&text),
            None,
            None,
            None,
            None,
        )];
    }

    let Some(obj) = value.as_object() else {
        return Vec::new();
    };

    match obj.get("type").and_then(Value::as_str) {
        Some("function_call") => responses_input_function_call_message(obj)
            .map(|message| vec![message])
            .unwrap_or_default(),
        Some("function_call_output") => responses_input_function_call_output_message(obj)
            .map(|message| vec![message])
            .unwrap_or_default(),
        Some("message") | None => {
            let role = obj.get("role").and_then(Value::as_str).unwrap_or("user");
            let content = obj
                .get("content")
                .and_then(value_to_text)
                .or_else(|| obj.get("text").and_then(Value::as_str).map(str::to_string))
                .unwrap_or_default();
            if content.is_empty() {
                Vec::new()
            } else {
                vec![request_message(
                    RequestMessageKind::Role,
                    Some(role),
                    Some(&content),
                    None,
                    None,
                    None,
                    None,
                )]
            }
        }
        Some("input_text") => obj
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(|text| {
                vec![request_message(
                    RequestMessageKind::User,
                    None,
                    Some(text),
                    None,
                    None,
                    None,
                    None,
                )]
            })
            .unwrap_or_default(),
        Some("output_text") => obj
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(|text| {
                vec![request_message(
                    RequestMessageKind::Role,
                    Some("assistant"),
                    Some(text),
                    None,
                    None,
                    None,
                    None,
                )]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn responses_input_function_call_message(obj: &serde_json::Map<String, Value>) -> Option<Value> {
    let call_id = obj
        .get("call_id")
        .or_else(|| obj.get("tool_call_id"))
        .or_else(|| obj.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("call_1");
    let name = obj
        .get("name")
        .or_else(|| obj.get("tool_name"))
        .or_else(|| {
            obj.get("function")
                .and_then(|function| function.get("name"))
        })
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())?;
    let namespace = obj
        .get("namespace")
        .and_then(Value::as_str)
        .filter(|ns| !ns.trim().is_empty());
    let arguments = obj
        .get("arguments")
        .or_else(|| obj.get("input"))
        .or_else(|| {
            obj.get("function")
                .and_then(|function| function.get("arguments"))
        })
        .map(stringify_arguments)
        .unwrap_or_else(|| "{}".to_string());
    Some(request_message(
        RequestMessageKind::FunctionCall,
        None,
        None,
        Some(call_id),
        namespace,
        Some(name),
        Some(&arguments),
    ))
}

fn responses_input_function_call_output_message(
    obj: &serde_json::Map<String, Value>,
) -> Option<Value> {
    let call_id = obj
        .get("call_id")
        .or_else(|| obj.get("tool_call_id"))
        .or_else(|| obj.get("id"))
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())?;
    let content = obj
        .get("output")
        .or_else(|| obj.get("content"))
        .or_else(|| obj.get("result"))
        .or_else(|| obj.get("error"))
        .map(stringify_arguments)
        .unwrap_or_default();
    Some(request_message(
        RequestMessageKind::FunctionCallOutput,
        None,
        Some(&content),
        Some(call_id),
        None,
        None,
        None,
    ))
}
