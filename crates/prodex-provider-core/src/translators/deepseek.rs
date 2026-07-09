use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
    ProviderUnsupportedReason,
};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy)]
pub struct DeepSeekTranslator;

impl ProviderTranslator for DeepSeekTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::DeepSeek
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiChatCompletions
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        if matches!(
            endpoint,
            ProviderEndpoint::Responses
                | ProviderEndpoint::ChatCompletions
                | ProviderEndpoint::Messages
        ) {
            return ProviderParamSupport {
                supported: true,
                unsupported: vec![
                    ProviderUnsupportedReason {
                        field: "parallel_tool_calls=false".to_string(),
                        reason:
                            "DeepSeek does not expose a compatible parallel tool disable control"
                                .to_string(),
                    },
                    ProviderUnsupportedReason {
                        field: "web_search_options".to_string(),
                        reason: "DeepSeek v1 translator does not map Responses web search options"
                            .to_string(),
                    },
                    ProviderUnsupportedReason {
                        field: "safety_identifier".to_string(),
                        reason:
                            "DeepSeek v1 translator does not forward Responses safety_identifier"
                                .to_string(),
                    },
                    ProviderUnsupportedReason {
                        field: "tools[type!=function]".to_string(),
                        reason:
                            "DeepSeek v1 translator only forwards plain function tools on Responses"
                                .to_string(),
                    },
                    ProviderUnsupportedReason {
                        field: "input[*].content[type!=text]".to_string(),
                        reason:
                            "DeepSeek v1 translator does not translate multimodal Responses input"
                                .to_string(),
                    },
                ],
            };
        }
        let mut support = ProviderParamSupport::full();
        if endpoint != ProviderEndpoint::Responses {
            support.supported = false;
            support.unsupported.push(ProviderUnsupportedReason {
                field: endpoint.label().to_string(),
                reason: "v1 translator currently models only responses compatibility".to_string(),
            });
        }
        support
    }

    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if deepseek_passthrough_endpoint(input.endpoint) {
            return ProviderTransformResult::lossless(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                input.body,
            );
        }
        if input.endpoint != ProviderEndpoint::Responses {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                format!(
                    "DeepSeek translator does not support {}",
                    input.endpoint.label()
                ),
            );
        }
        let value: Value = match serde_json::from_slice(&input.body) {
            Ok(value) => value,
            Err(error) => {
                return ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.client_wire_format(),
                    self.upstream_wire_format(),
                    format!("failed to parse Responses request JSON: {error}"),
                );
            }
        };
        let Some(obj) = value.as_object() else {
            return ProviderTransformResult::rejected(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                "DeepSeek request body must be a JSON object",
            );
        };
        if matches!(
            obj.get("parallel_tool_calls").and_then(Value::as_bool),
            Some(false)
        ) {
            return ProviderTransformResult::rejected(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                "DeepSeek does not expose a compatible parallel_tool_calls=false control",
            );
        }
        let mut request = serde_json::Map::new();
        request.insert(
            "model".to_string(),
            Value::String(
                obj.get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("deepseek-chat")
                    .to_string(),
            ),
        );
        request.insert(
            "stream".to_string(),
            Value::Bool(obj.get("stream").and_then(Value::as_bool).unwrap_or(false)),
        );
        let messages = deepseek_messages_from_request(&value);
        request.insert("messages".to_string(), Value::Array(messages));
        if let Some(tools) = obj.get("tools").and_then(Value::as_array) {
            let function_tools: Vec<Value> = tools
                .iter()
                .filter(|tool| tool.get("type").and_then(Value::as_str) == Some("function"))
                .cloned()
                .collect();
            if !function_tools.is_empty() {
                request.insert("tools".to_string(), Value::Array(function_tools));
            }
        }
        if let Some(tool_choice) = deepseek_tool_choice_from_request(&value) {
            request.insert("tool_choice".to_string(), tool_choice);
        }
        if let Err(reason) = deepseek_insert_primitive_request_fields(&value, &mut request) {
            return ProviderTransformResult::rejected(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                reason,
            );
        }
        match deepseek_top_logprobs_from_request(&value) {
            Ok(Some(top_logprobs)) => {
                request.insert("top_logprobs".to_string(), top_logprobs);
            }
            Ok(None) => {}
            Err(reason) => {
                return ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.client_wire_format(),
                    self.upstream_wire_format(),
                    reason,
                );
            }
        }
        match deepseek_stop_from_request(&value) {
            Ok(Some(stop)) => {
                request.insert("stop".to_string(), stop);
            }
            Ok(None) => {}
            Err(reason) => {
                return ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.client_wire_format(),
                    self.upstream_wire_format(),
                    reason,
                );
            }
        }
        match deepseek_user_id_from_request(&value) {
            Ok(Some(user_id)) => {
                request.insert("user_id".to_string(), Value::String(user_id));
            }
            Ok(None) => {}
            Err(reason) => {
                return ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.client_wire_format(),
                    self.upstream_wire_format(),
                    reason,
                );
            }
        }
        let mut degraded = None;
        if let Some(response_format) = obj.get("response_format") {
            match response_format
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("text")
            {
                "text" => {}
                "json_object" => {
                    request.insert(
                        "response_format".to_string(),
                        json!({"type": "json_object"}),
                    );
                }
                "json_schema" | "json" | "structured_output" => {
                    request.insert(
                        "response_format".to_string(),
                        json!({"type": "json_object"}),
                    );
                    degraded = Some({
                        let mut map = BTreeMap::new();
                        map.insert("from".to_string(), Value::String("json_schema".to_string()));
                        map.insert("to".to_string(), Value::String("json_object".to_string()));
                        map
                    });
                }
                other => {
                    return ProviderTransformResult::rejected(
                        self.provider(),
                        input.endpoint,
                        self.client_wire_format(),
                        self.upstream_wire_format(),
                        format!("DeepSeek response_format type `{other}` is not supported"),
                    );
                }
            }
        }
        let body =
            serde_json::to_vec(&Value::Object(request)).expect("deepseek request serializes");
        let result = if let Some(details) = degraded {
            ProviderTransformResult::degraded(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                body,
                "DeepSeek degrades JSON schema output to json_object",
                details,
            )
        } else {
            ProviderTransformResult::lossless(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                body,
            )
        };
        let mut metadata = serde_json::Map::new();
        for header in ["x-codex-turn-state", "session_id"] {
            if let Some(value) = input.headers.get(header) {
                metadata.insert(header.to_string(), Value::String(value.clone()));
            }
        }
        if let Some(previous) = obj.get("previous_response_id").and_then(Value::as_str) {
            metadata.insert(
                "previous_response_id".to_string(),
                Value::String(previous.to_string()),
            );
        }
        if metadata.is_empty() {
            result
        } else {
            result.with_metadata("continuation", Value::Object(metadata))
        }
    }

    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if deepseek_passthrough_endpoint(input.endpoint) {
            return ProviderTransformResult::lossless(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                input.body,
            );
        }
        if input.endpoint != ProviderEndpoint::Responses {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                format!(
                    "DeepSeek translator does not support {}",
                    input.endpoint.label()
                ),
            );
        }
        let value: Value = match serde_json::from_slice(&input.body) {
            Ok(value) => value,
            Err(error) => {
                return ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.upstream_wire_format(),
                    self.client_wire_format(),
                    format!("failed to parse DeepSeek response JSON: {error}"),
                );
            }
        };
        let response = deepseek_responses_value_from_chat_value(&value);
        ProviderTransformResult::lossless(
            self.provider(),
            input.endpoint,
            self.upstream_wire_format(),
            self.client_wire_format(),
            serde_json::to_vec(&response).expect("deepseek response serializes"),
        )
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if deepseek_passthrough_endpoint(input.endpoint) {
            return ProviderTransformResult::lossless(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                input.body,
            );
        }
        if input.endpoint != ProviderEndpoint::Responses {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                format!(
                    "DeepSeek translator does not support {}",
                    input.endpoint.label()
                ),
            );
        }
        let event = String::from_utf8_lossy(&input.body);
        let Some(data) = event
            .strip_prefix("data: ")
            .and_then(|s| s.strip_suffix("\n\n"))
        else {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                "DeepSeek SSE event must use data: <json> framing",
            );
        };
        if data == "[DONE]" {
            return ProviderTransformResult::lossless(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                b"event: response.completed\ndata: {}\n\n".to_vec(),
            );
        }
        let value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(error) => {
                return ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.upstream_wire_format(),
                    self.client_wire_format(),
                    format!("failed to parse DeepSeek SSE JSON: {error}"),
                );
            }
        };
        let Some((event_name, transformed)) = deepseek_stream_event_from_chat_value(&value) else {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                "DeepSeek SSE event does not contain a supported text or function-call delta",
            );
        };
        let body = format!("event: {event_name}\ndata: {}\n\n", transformed);
        ProviderTransformResult::lossless(
            self.provider(),
            input.endpoint,
            self.upstream_wire_format(),
            self.client_wire_format(),
            body.into_bytes(),
        )
    }
}

fn deepseek_stream_event_from_chat_value(value: &Value) -> Option<(&'static str, Value)> {
    let delta = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))?;
    if let Some(tool_call) = delta
        .get("tool_calls")
        .and_then(Value::as_array)
        .and_then(|tool_calls| tool_calls.first())
    {
        let arguments = tool_call
            .get("function")
            .and_then(|function| function.get("arguments"))
            .and_then(Value::as_str)?;
        let mut transformed = json!({
            "type":"response.function_call_arguments.delta",
            "delta": arguments,
        });
        if let Some(call_id) = tool_call.get("id").and_then(Value::as_str)
            && let Some(object) = transformed.as_object_mut()
        {
            object.insert("call_id".to_string(), Value::String(call_id.to_string()));
        }
        return Some(("response.function_call_arguments.delta", transformed));
    }
    let text = delta
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Some((
        "response.output_text.delta",
        if text.is_empty() {
            json!({"type":"response.output_text.delta","delta":""})
        } else {
            json!({"type":"response.output_text.delta","delta":text})
        },
    ))
}

fn deepseek_passthrough_endpoint(endpoint: ProviderEndpoint) -> bool {
    matches!(
        endpoint,
        ProviderEndpoint::ChatCompletions | ProviderEndpoint::Messages
    )
}

fn deepseek_tool_choice_from_request(value: &Value) -> Option<Value> {
    let choice = value.get("tool_choice")?;
    if let Some(choice) = choice.as_str() {
        return matches!(choice, "auto" | "none" | "required")
            .then(|| Value::String(choice.to_string()));
    }
    let object = choice.as_object()?;
    let choice_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if choice_type != "function" {
        return None;
    }
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| {
            object
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
        })
        .filter(|name| !name.trim().is_empty())?;
    Some(json!({
        "type": "function",
        "function": {
            "name": name,
        },
    }))
}

fn deepseek_insert_primitive_request_fields(
    value: &Value,
    request: &mut serde_json::Map<String, Value>,
) -> Result<(), String> {
    for field in ["temperature", "top_p"] {
        if let Some(next) = value.get(field) {
            if !next.is_number() {
                return Err(format!("DeepSeek {field} must be a number"));
            }
            request.insert(field.to_string(), next.clone());
        }
    }
    for field in ["max_output_tokens", "max_tokens", "max_completion_tokens"] {
        if let Some(next) = value.get(field) {
            if next.as_u64().is_none_or(|count| count == 0) {
                return Err(format!("DeepSeek {field} must be a positive integer"));
            }
            request.insert("max_tokens".to_string(), next.clone());
        }
    }
    if let Some(logprobs) = value.get("logprobs") {
        if !logprobs.is_boolean() {
            return Err("DeepSeek logprobs must be a boolean".to_string());
        }
        request.insert("logprobs".to_string(), logprobs.clone());
    }
    Ok(())
}

fn deepseek_top_logprobs_from_request(value: &Value) -> Result<Option<Value>, String> {
    let Some(top_logprobs) = value.get("top_logprobs") else {
        return Ok(None);
    };
    let Some(count) = top_logprobs.as_u64() else {
        return Err("DeepSeek top_logprobs must be an integer".to_string());
    };
    if count > 20 {
        return Err("DeepSeek top_logprobs must be <= 20".to_string());
    }
    if value.get("logprobs").and_then(Value::as_bool) != Some(true) {
        return Err("DeepSeek top_logprobs requires logprobs=true".to_string());
    }
    Ok(Some(top_logprobs.clone()))
}

fn deepseek_stop_from_request(value: &Value) -> Result<Option<Value>, String> {
    let Some(stop) = value
        .get("stop")
        .or_else(|| value.get("stop_sequences"))
        .or_else(|| value.get("stopSequences"))
    else {
        return Ok(None);
    };
    if stop.as_str().is_some() {
        return Ok(Some(stop.clone()));
    }
    let Some(stops) = stop.as_array() else {
        return Err("DeepSeek stop must be a string or array of strings".to_string());
    };
    if stops.len() > 16 {
        return Err("DeepSeek supports at most 16 stop sequences".to_string());
    }
    if stops.iter().any(|stop| !stop.is_string()) {
        return Err("DeepSeek stop sequences must be strings".to_string());
    }
    Ok(Some(stop.clone()))
}

fn deepseek_user_id_from_request(value: &Value) -> Result<Option<String>, String> {
    let Some(user_id) = value
        .get("user_id")
        .or_else(|| value.get("user"))
        .or_else(|| value.get("safety_identifier"))
    else {
        return Ok(None);
    };
    let Some(user_id) = user_id.as_str() else {
        return Err("DeepSeek user_id must be a string".to_string());
    };
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Ok(None);
    }
    if user_id.len() > 512
        || !user_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(
            "DeepSeek user_id must use only letters, numbers, underscores, or dashes and be at most 512 bytes".to_string(),
        );
    }
    Ok(Some(user_id.to_string()))
}

fn deepseek_messages_from_request(value: &Value) -> Vec<Value> {
    let mut messages = if let Some(messages) = value.get("messages").and_then(Value::as_array) {
        messages.clone()
    } else if let Some(input) = value.get("input") {
        match input {
            Value::String(text) => vec![json!({"role":"user","content": text})],
            Value::Array(items) => items
                .iter()
                .flat_map(deepseek_messages_from_input_item)
                .collect(),
            _ => vec![json!({"role":"user","content":""})],
        }
    } else {
        vec![json!({"role":"user","content":""})]
    };
    if let Some(instructions) = value
        .get("instructions")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
    {
        messages.insert(
            0,
            json!({
                "role": "system",
                "content": instructions,
            }),
        );
    }
    messages
}

fn deepseek_messages_from_input_item(item: &Value) -> Vec<Value> {
    match item.get("type").and_then(Value::as_str) {
        Some("function_call") => {
            return vec![
                deepseek_input_function_call_message(item)
                    .unwrap_or_else(|| json!({"role":"assistant","content":""})),
            ];
        }
        Some("mcp_call") => {
            return deepseek_input_mcp_call_messages(item)
                .unwrap_or_else(|| vec![json!({"role":"assistant","content":""})]);
        }
        Some("custom_tool_call") => {
            return vec![
                deepseek_input_custom_tool_call_message(item)
                    .unwrap_or_else(|| json!({"role":"assistant","content":""})),
            ];
        }
        Some("local_shell_call") => {
            return vec![
                deepseek_input_local_shell_call_message(item)
                    .unwrap_or_else(|| json!({"role":"assistant","content":""})),
            ];
        }
        Some("function_call_output") => {
            return vec![json!({
                "role": "tool",
                "tool_call_id": deepseek_input_item_call_id(item),
                "content": deepseek_tool_output_text(item),
            })];
        }
        Some("custom_tool_call_output") => {
            return vec![json!({
                "role": "tool",
                "tool_call_id": deepseek_input_item_call_id(item),
                "content": deepseek_tool_output_text(item),
            })];
        }
        Some("mcp_tool_result") | Some("mcp_call_output") => {
            return vec![json!({
                "role": "tool",
                "tool_call_id": deepseek_input_item_call_id(item),
                "content": deepseek_tool_output_text(item),
            })];
        }
        _ => {}
    }
    let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
    let content = item
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| deepseek_message_content_text(item.get("content")))
        .or_else(|| item.get("text").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    if role == "tool" {
        let mut message = json!({
            "role": "tool",
            "content": content,
        });
        if let Some(call_id) = item.get("tool_call_id").and_then(Value::as_str)
            && let Some(object) = message.as_object_mut()
        {
            object.insert(
                "tool_call_id".to_string(),
                Value::String(call_id.to_string()),
            );
        }
        return vec![message];
    }
    let tool_calls = deepseek_message_tool_calls(item);
    let mut message = json!({
        "role": role,
        "content": content,
    });
    if let Some(tool_calls) = tool_calls
        && let Some(object) = message.as_object_mut()
    {
        object.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    vec![message]
}

fn deepseek_input_item_call_id(item: &Value) -> &str {
    item.get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .filter(|call_id| !call_id.trim().is_empty())
        .unwrap_or("call_1")
}

fn deepseek_message_content_text(value: Option<&Value>) -> Option<String> {
    let parts = value?.as_array()?;
    let text = parts
        .iter()
        .filter_map(deepseek_message_content_part_text)
        .collect::<Vec<_>>()
        .join("\n");
    Some(text)
}

fn deepseek_message_content_part_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(object) => object
            .get("text")
            .and_then(Value::as_str)
            .or_else(|| object.get("input_text").and_then(Value::as_str))
            .or_else(|| object.get("output_text").and_then(Value::as_str))
            .map(str::to_string),
        _ => None,
    }
}

fn deepseek_message_tool_calls(item: &Value) -> Option<Vec<Value>> {
    let tool_calls = item.get("tool_calls")?.as_array()?;
    let translated = tool_calls
        .iter()
        .filter_map(deepseek_message_tool_call)
        .collect::<Vec<_>>();
    (!translated.is_empty()).then_some(translated)
}

fn deepseek_message_tool_call(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let function = object.get("function").and_then(Value::as_object)?;
    let name = function.get("name").and_then(Value::as_str)?;
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            function
                .get("arguments")
                .map(|arguments| arguments.to_string())
        })
        .unwrap_or_else(|| "{}".to_string());
    let mut tool_call = json!({
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    if let Some(call_id) = object.get("id").and_then(Value::as_str)
        && let Some(tool_call_object) = tool_call.as_object_mut()
    {
        tool_call_object.insert("id".to_string(), Value::String(call_id.to_string()));
    }
    Some(tool_call)
}

fn deepseek_input_function_call_message(item: &Value) -> Option<Value> {
    let call_id = item
        .get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("call_1");
    let name = item
        .get("name")
        .or_else(|| item.get("tool_name"))
        .or_else(|| {
            item.get("function")
                .and_then(|function| function.get("name"))
        })
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())?;
    let arguments = item
        .get("arguments")
        .or_else(|| item.get("input"))
        .or_else(|| {
            item.get("function")
                .and_then(|function| function.get("arguments"))
        })
        .map(deepseek_stringified_arguments)
        .unwrap_or_else(|| "{}".to_string());
    let mut tool_call = json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    if let Some(signature) = deepseek_tool_call_thought_signature(item)
        && let Some(tool_call_object) = tool_call.as_object_mut()
    {
        tool_call_object.insert(
            "gemini_thought_signature".to_string(),
            Value::String(signature),
        );
    }
    Some(json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [tool_call],
    }))
}

fn deepseek_input_custom_tool_call_message(item: &Value) -> Option<Value> {
    let call_id = item
        .get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("call_1");
    let name = item
        .get("name")
        .or_else(|| item.get("tool_name"))
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())?;
    let input = item
        .get("input")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| deepseek_message_content_text(item.get("input")))
        .or_else(|| item.get("input").map(deepseek_stringified_arguments))
        .unwrap_or_default();
    let arguments = serde_json::to_string(&json!({ "input": input })).ok()?;
    let mut tool_call = json!({
        "id": call_id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": arguments,
        },
    });
    if let Some(signature) = deepseek_tool_call_thought_signature(item)
        && let Some(tool_call_object) = tool_call.as_object_mut()
    {
        tool_call_object.insert(
            "gemini_thought_signature".to_string(),
            Value::String(signature),
        );
    }
    Some(json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [tool_call],
    }))
}

fn deepseek_input_local_shell_call_message(item: &Value) -> Option<Value> {
    let call_id = item
        .get("call_id")
        .or_else(|| item.get("tool_call_id"))
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("call_1");
    let command = item
        .get("action")
        .and_then(|action| action.get("command"))
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|command| !command.trim().is_empty())
        .or_else(|| {
            item.get("command")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|command| !command.trim().is_empty())?;
    let mut shell_arguments = serde_json::Map::new();
    shell_arguments.insert("command".to_string(), Value::String(command));
    deepseek_copy_shell_argument(item, &mut shell_arguments, "cwd");
    deepseek_copy_shell_argument(item, &mut shell_arguments, "timeout");
    deepseek_copy_shell_argument(item, &mut shell_arguments, "env");
    let arguments = serde_json::to_string(&Value::Object(shell_arguments)).ok()?;
    Some(json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [{
            "id": call_id,
            "type": "function",
            "function": {
                "name": "shell_command",
                "arguments": arguments,
            },
        }],
    }))
}

fn deepseek_input_mcp_call_messages(item: &Value) -> Option<Vec<Value>> {
    let assistant = deepseek_input_function_call_message(item)?;
    let mut messages = vec![assistant];
    if deepseek_mcp_call_has_result(item) {
        let call_id = item
            .get("call_id")
            .or_else(|| item.get("tool_call_id"))
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .unwrap_or("call_1");
        messages.push(json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": deepseek_tool_output_text(item),
        }));
    }
    Some(messages)
}

fn deepseek_tool_call_thought_signature(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    deepseek_string_field(
        object,
        &[
            "gemini_thought_signature",
            "thought_signature",
            "thoughtSignature",
        ],
    )
    .or_else(|| {
        object
            .get("provider_specific_fields")
            .and_then(Value::as_object)
            .and_then(|nested| nested.get("thought_signature"))
            .and_then(Value::as_str)
            .map(str::to_string)
    })
    .or_else(|| {
        object
            .get("extra_content")
            .and_then(|value| value.get("google"))
            .and_then(|value| value.get("thought_signature"))
            .and_then(Value::as_str)
            .map(str::to_string)
    })
    .filter(|signature| !signature.trim().is_empty())
}

fn deepseek_string_field(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn deepseek_stringified_arguments(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn deepseek_copy_shell_argument(
    item: &Value,
    arguments: &mut serde_json::Map<String, Value>,
    key: &str,
) {
    if let Some(value) = item
        .get(key)
        .or_else(|| item.get("action").and_then(|action| action.get(key)))
    {
        arguments.insert(key.to_string(), value.clone());
    }
}

fn deepseek_tool_output_value(item: &Value) -> Value {
    item.get("output")
        .or_else(|| item.get("content"))
        .or_else(|| item.get("result"))
        .or_else(|| item.get("error"))
        .cloned()
        .unwrap_or_else(|| Value::String(String::new()))
}

fn deepseek_tool_output_text(item: &Value) -> String {
    let value = deepseek_tool_output_value(item);
    deepseek_content_value_text(&value)
}

fn deepseek_content_value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(deepseek_message_content_part_text)
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}

fn deepseek_mcp_call_has_result(item: &Value) -> bool {
    item.get("output").is_some()
        || item.get("content").is_some()
        || item.get("result").is_some()
        || item.get("error").is_some()
}

fn deepseek_responses_value_from_chat_value(value: &Value) -> Value {
    let response_id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("chatcmpl_prodex");
    let created_at = value
        .get("created")
        .and_then(Value::as_u64)
        .unwrap_or_else(deepseek_created_at);
    let message = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"));
    let mut output = Vec::new();
    let mut tool_call_error = None;
    if let Some(text) = message
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        output.push(json!({
            "type":"message",
            "role":"assistant",
            "content":[{"type":"output_text","text":text}],
        }));
    }
    if let Some(tool_calls) = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(Value::as_array)
    {
        for tool_call in tool_calls {
            match deepseek_responses_tool_call_item(tool_call) {
                Ok(Some(item)) => output.push(item),
                Ok(None) => {}
                Err(error) => {
                    tool_call_error = Some(error);
                    break;
                }
            }
        }
    }
    let mut response = json!({
        "id": response_id,
        "object": "response",
        "created_at": created_at,
        "model": value.get("model").and_then(Value::as_str).unwrap_or("deepseek-chat"),
        "output": output,
    });
    if let Some(error) = tool_call_error {
        response["status"] = Value::String("failed".to_string());
        response["error"] = json!({
            "code": "invalid_tool_call_arguments",
            "message": error,
        });
    }
    if let Some(usage) = value.get("usage").and_then(deepseek_responses_usage) {
        response["usage"] = usage;
    }
    let mut metadata = serde_json::Map::new();
    if let Some(logprobs) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("logprobs"))
        .filter(|logprobs| !logprobs.is_null())
    {
        metadata.insert("logprobs".to_string(), logprobs.clone());
    }
    if let Some(reasoning_content) = message
        .and_then(|message| message.get("reasoning_content"))
        .and_then(Value::as_str)
        .filter(|reasoning_content| !reasoning_content.is_empty())
    {
        metadata.insert(
            "reasoning_content".to_string(),
            Value::String(reasoning_content.to_string()),
        );
    }
    if let Some(refusal) = message
        .and_then(|message| message.get("refusal"))
        .and_then(Value::as_str)
        .filter(|refusal| !refusal.is_empty())
    {
        metadata.insert("refusal".to_string(), Value::String(refusal.to_string()));
    }
    if let Some(annotations) = message
        .and_then(|message| message.get("annotations"))
        .and_then(Value::as_array)
        .filter(|annotations| !annotations.is_empty())
    {
        metadata.insert("annotations".to_string(), Value::Array(annotations.clone()));
    }
    if let Some(finish_reason) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(Value::as_str)
    {
        metadata.insert(
            "finish_reason".to_string(),
            Value::String(finish_reason.to_string()),
        );
    }
    if let Some(system_fingerprint) = value
        .get("system_fingerprint")
        .and_then(Value::as_str)
        .filter(|system_fingerprint| !system_fingerprint.is_empty())
    {
        metadata.insert(
            "system_fingerprint".to_string(),
            Value::String(system_fingerprint.to_string()),
        );
    }
    if !metadata.is_empty() {
        response["metadata"] = json!({ "deepseek": metadata });
    }
    response
}

fn deepseek_responses_tool_call_item(tool_call: &Value) -> Result<Option<Value>, String> {
    let call_id = tool_call
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("call_0");
    let Some(function) = tool_call.get("function").and_then(Value::as_object) else {
        return Err("DeepSeek returned a tool call without a function object".to_string());
    };
    let Some(name) = function
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
    else {
        return Err("DeepSeek returned a tool call without a function name".to_string());
    };
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    deepseek_validate_tool_call_arguments(name, arguments)?;
    if name == "tool_search" {
        let arguments = serde_json::from_str::<Value>(arguments)
            .map_err(|error| deepseek_tool_call_arguments_error(name, error))?;
        return Ok(Some(json!({
            "type": "tool_search_call",
            "call_id": call_id,
            "execution": "client",
            "arguments": arguments,
        })));
    }
    if name == "apply_patch" {
        return Ok(Some(json!({
            "type": "custom_tool_call",
            "call_id": call_id,
            "name": name,
            "input": arguments,
        })));
    }
    let arguments = deepseek_rtk_wrapped_tool_arguments(name, arguments);
    let (namespace, name) = deepseek_split_flat_namespace_tool_name(name);
    let mut item = json!({
        "type":"function_call",
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
    });
    if let Some(namespace) = namespace {
        item["namespace"] = Value::String(namespace);
    }
    if let Some(signature) = deepseek_chat_tool_call_thought_signature(tool_call) {
        item["gemini_thought_signature"] = Value::String(signature);
    }
    Ok(Some(item))
}

fn deepseek_validate_tool_call_arguments(name: &str, arguments: &str) -> Result<(), String> {
    if arguments.trim().is_empty() {
        return Ok(());
    }
    serde_json::from_str::<Value>(arguments)
        .map(|_| ())
        .map_err(|error| deepseek_tool_call_arguments_error(name, error))
}

fn deepseek_tool_call_arguments_error(name: &str, error: serde_json::Error) -> String {
    format!("DeepSeek returned malformed JSON arguments for tool call `{name}`: {error}")
}

fn deepseek_chat_tool_call_thought_signature(tool_call: &Value) -> Option<String> {
    tool_call
        .get("extra_content")
        .and_then(|value| value.get("google"))
        .and_then(|value| value.get("thought_signature"))
        .and_then(Value::as_str)
        .filter(|signature| !signature.trim().is_empty())
        .map(str::to_string)
}

fn deepseek_responses_usage(usage: &Value) -> Option<Value> {
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    let cache_hit_tokens = usage.get("prompt_cache_hit_tokens").and_then(Value::as_u64);
    let cache_miss_tokens = usage
        .get("prompt_cache_miss_tokens")
        .and_then(Value::as_u64);
    let reasoning_tokens = usage
        .get("completion_tokens_details")
        .and_then(|details| details.get("reasoning_tokens"))
        .and_then(Value::as_u64);
    let mut response_usage = json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": total_tokens,
    });
    if let Some(cache_hit_tokens) = cache_hit_tokens {
        response_usage["input_tokens_details"] = json!({
            "cached_tokens": cache_hit_tokens,
        });
    }
    if let Some(reasoning_tokens) = reasoning_tokens {
        response_usage["output_tokens_details"] = json!({
            "reasoning_tokens": reasoning_tokens,
        });
    }
    if cache_hit_tokens.is_some() || cache_miss_tokens.is_some() {
        response_usage["metadata"] = json!({
            "deepseek": {
                "prompt_cache_hit_tokens": cache_hit_tokens.unwrap_or(0),
                "prompt_cache_miss_tokens": cache_miss_tokens.unwrap_or(0),
            }
        });
    }
    Some(response_usage)
}

fn deepseek_created_at() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn deepseek_split_flat_namespace_tool_name(name: &str) -> (Option<String>, String) {
    let trimmed = name.trim();
    for separator in ["__", ".", "/"] {
        if let Some((namespace, local)) = trimmed.split_once(separator)
            && !namespace.trim().is_empty()
            && !local.trim().is_empty()
        {
            return (Some(namespace.trim().to_string()), local.trim().to_string());
        }
    }
    (None, trimmed.to_string())
}

fn deepseek_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    if !matches!(name, "exec" | "functions.exec_command") || arguments.trim().is_empty() {
        return arguments.to_string();
    }
    let Ok(mut value) = serde_json::from_str::<Value>(arguments) else {
        return arguments.to_string();
    };
    let Some(cmd) = value
        .get_mut("cmd")
        .and_then(|value| value.as_str())
        .map(str::to_string)
    else {
        return arguments.to_string();
    };
    if cmd.trim().is_empty() || cmd.trim_start().starts_with("rtk ") {
        return arguments.to_string();
    }
    if let Some(object) = value.as_object_mut() {
        object.insert("cmd".to_string(), Value::String(format!("rtk {cmd}")));
    }
    serde_json::to_string(&value).unwrap_or_else(|_| arguments.to_string())
}
