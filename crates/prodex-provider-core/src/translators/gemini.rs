use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
    ProviderUnsupportedReason,
};
use crate::{
    ProviderEndpoint, ProviderId, ProviderTokenUsage, ProviderWireFormat, extract_usage_tokens,
};
use serde_json::{Value, json};
use std::borrow::Cow;

#[derive(Clone, Copy)]
pub struct GeminiTranslator;

impl ProviderTranslator for GeminiTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::Gemini
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::GeminiGenerateContent
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        if matches!(
            endpoint,
            ProviderEndpoint::Responses
                | ProviderEndpoint::ChatCompletions
                | ProviderEndpoint::Messages
                | ProviderEndpoint::Embeddings
        ) {
            return ProviderParamSupport {
                supported: true,
                unsupported: vec![
                    ProviderUnsupportedReason {
                        field: "input[*].content[type!=text]".to_string(),
                        reason:
                            "Gemini conformance v1 does not yet translate multimodal Responses input"
                                .to_string(),
                    },
                    ProviderUnsupportedReason {
                        field: "response_format.type".to_string(),
                        reason:
                            "Gemini v1 translator supports only text, json_object, and json_schema response formats"
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
        if gemini_passthrough_endpoint(input.endpoint) {
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
                    "Gemini translator does not support {}",
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
                "Gemini request body must be a JSON object",
            );
        };
        if contains_multimodal_input(&value) {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                "Gemini conformance v1 does not yet translate multimodal Responses input",
            );
        }
        let mut request = serde_json::Map::new();
        if let Some(system_instruction) = gemini_system_instruction_from_request(&value) {
            request.insert("systemInstruction".to_string(), system_instruction);
        }
        request.insert(
            "contents".to_string(),
            Value::Array(gemini_contents_from_request(&value)),
        );
        let model = obj
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("gemini-2.5-pro")
            .to_string();
        let mut generation_config = serde_json::Map::new();
        gemini_insert_basic_generation_config(obj, &mut generation_config);
        gemini_insert_extended_generation_config(obj, &mut generation_config);
        gemini_apply_text_format(obj, &mut generation_config);
        if let Some(thinking_config) = gemini_thinking_config_from_request(obj, &model) {
            generation_config.insert("thinkingConfig".to_string(), thinking_config);
        }
        if let Some(response_format) = obj.get("response_format") {
            match response_format
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("text")
            {
                "text" => {}
                "json_object" => {
                    generation_config.insert(
                        "responseMimeType".to_string(),
                        Value::String("application/json".to_string()),
                    );
                }
                "json_schema" => {
                    generation_config.insert(
                        "responseMimeType".to_string(),
                        Value::String("application/json".to_string()),
                    );
                    if let Some(schema) = response_format.get("schema") {
                        generation_config
                            .insert("responseJsonSchema".to_string(), sanitize_schema(schema));
                    }
                }
                other => {
                    return ProviderTransformResult::rejected(
                        self.provider(),
                        input.endpoint,
                        self.client_wire_format(),
                        self.upstream_wire_format(),
                        format!("Gemini response_format type `{other}` is not supported"),
                    );
                }
            }
        }
        if !generation_config.is_empty() {
            request.insert(
                "generationConfig".to_string(),
                Value::Object(generation_config),
            );
        }
        if let Some(tools) = obj.get("tools").and_then(Value::as_array) {
            let mut translated_tools = gemini_builtin_tools_from_request(tools);
            let declarations: Vec<Value> = tools
                .iter()
                .filter_map(gemini_tool_from_openai_tool)
                .collect();
            if !declarations.is_empty() {
                translated_tools.push(json!({"functionDeclarations": declarations}));
            }
            if !translated_tools.is_empty() {
                request.insert("tools".to_string(), Value::Array(translated_tools));
            }
        }
        if let Some(tool_config) = gemini_tool_config_from_request(&value) {
            request.insert("toolConfig".to_string(), tool_config);
        }
        if let Some(settings) = obj
            .get("safety_settings")
            .or_else(|| obj.get("safetySettings"))
        {
            request.insert("safetySettings".to_string(), settings.clone());
        }
        if let Some(cached_content) = obj
            .get("cached_content")
            .or_else(|| obj.get("cachedContent"))
            .filter(|value| !value.is_null())
        {
            request.insert("cachedContent".to_string(), cached_content.clone());
        }
        if let Some(labels) = obj.get("labels").filter(|value| !value.is_null()) {
            request.insert("labels".to_string(), labels.clone());
        }
        let body = serde_json::to_vec(&json!({"model": model, "request": Value::Object(request)}))
            .expect("gemini request serializes");
        let result = ProviderTransformResult::lossless(
            self.provider(),
            input.endpoint,
            self.client_wire_format(),
            self.upstream_wire_format(),
            body,
        );
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
        if gemini_passthrough_endpoint(input.endpoint) {
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
                    "Gemini translator does not support {}",
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
                    format!("failed to parse Gemini response JSON: {error}"),
                );
            }
        };
        let value = gemini_normalized_response_value(&value);
        let response = gemini_responses_value_from_generate_value(&value);
        ProviderTransformResult::lossless(
            self.provider(),
            input.endpoint,
            self.upstream_wire_format(),
            self.client_wire_format(),
            serde_json::to_vec(&response).expect("gemini response serializes"),
        )
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if gemini_passthrough_endpoint(input.endpoint) {
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
                    "Gemini translator does not support {}",
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
                "Gemini SSE event must use data: <json> framing",
            );
        };
        let value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(error) => {
                return ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.upstream_wire_format(),
                    self.client_wire_format(),
                    format!("failed to parse Gemini SSE JSON: {error}"),
                );
            }
        };
        let Some((event_name, transformed)) = gemini_stream_event_from_generate_value(&value)
        else {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                "Gemini SSE event does not contain a supported text or function-call delta",
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

    fn extract_usage(&self, body: &[u8]) -> ProviderTokenUsage {
        let Ok(value) = serde_json::from_slice::<Value>(body) else {
            return extract_usage_tokens(body);
        };
        let value = gemini_normalized_response_value(&value);
        extract_usage_tokens(&serde_json::to_vec(value.as_ref()).unwrap_or_else(|_| body.to_vec()))
    }
}

fn gemini_stream_event_from_generate_value(value: &Value) -> Option<(&'static str, Value)> {
    if let Some(function_call) = value.pointer("/candidates/0/content/parts/0/functionCall") {
        let args = function_call
            .get("args")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut transformed = json!({
            "type":"response.function_call_arguments.delta",
            "delta": serde_json::to_string(&args).ok()?,
        });
        if let Some(call_id) = function_call.get("id").and_then(Value::as_str)
            && let Some(object) = transformed.as_object_mut()
        {
            object.insert("call_id".to_string(), Value::String(call_id.to_string()));
        }
        return Some(("response.function_call_arguments.delta", transformed));
    }
    if let Some(part) = value.pointer("/candidates/0/content/parts/0")
        && part
            .get("thought")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        let text = part.get("text").and_then(Value::as_str)?;
        return Some((
            "response.reasoning_summary_text.delta",
            json!({
                "type":"response.reasoning_summary_text.delta",
                "delta":text,
            }),
        ));
    }
    let text = value
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(Value::as_str)?;
    Some((
        "response.output_text.delta",
        json!({"type":"response.output_text.delta","delta":text}),
    ))
}

fn gemini_passthrough_endpoint(endpoint: ProviderEndpoint) -> bool {
    matches!(
        endpoint,
        ProviderEndpoint::ChatCompletions
            | ProviderEndpoint::Messages
            | ProviderEndpoint::Embeddings
    )
}

fn contains_multimodal_input(value: &Value) -> bool {
    value
        .get("input")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().any(|item| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("input_image") | Some("input_audio") | Some("input_file")
                )
            })
        })
        .unwrap_or(false)
}

fn gemini_system_instruction_from_request(value: &Value) -> Option<Value> {
    let items = value.get("input")?.as_array()?;
    let mut system_text = items
        .iter()
        .filter(|item| item.get("role").and_then(Value::as_str) == Some("system"))
        .filter_map(gemini_message_text)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let contextual_user_text = items
        .iter()
        .filter_map(gemini_contextual_user_instruction_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    if !contextual_user_text.is_empty() {
        if !system_text.is_empty() {
            system_text.push_str("\n\n");
        }
        system_text.push_str(&contextual_user_text);
    }
    (!system_text.trim().is_empty()).then(|| json!({ "parts": [{ "text": system_text }] }))
}

fn gemini_contents_from_request(value: &Value) -> Vec<Value> {
    if let Some(input) = value.get("input") {
        return match input {
            Value::String(text) => vec![json!({"role":"user","parts":[{"text": text}]})],
            Value::Array(items) => gemini_contents_from_input_items(items),
            _ => vec![json!({"role":"user","parts":[{"text":""}]})],
        };
    }
    vec![json!({"role":"user","parts":[{"text":""}]})]
}

fn gemini_contents_from_input_items(items: &[Value]) -> Vec<Value> {
    let mut contents = Vec::new();
    let mut tool_names_by_call_id = std::collections::BTreeMap::new();
    let mut index = 0;
    while index < items.len() {
        let item = &items[index];
        let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
        match role {
            "system" => {}
            "assistant" => {
                let mut parts = Vec::new();
                if let Some(text) = gemini_message_text(item).filter(|text| !text.is_empty()) {
                    parts.push(json!({ "text": text }));
                }
                if let Some(tool_calls) = item.get("tool_calls").and_then(Value::as_array) {
                    for tool_call in tool_calls {
                        let call_id = tool_call
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let Some(function) = tool_call.get("function") else {
                            continue;
                        };
                        let name = function
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("tool_call");
                        if !call_id.is_empty() {
                            tool_names_by_call_id.insert(call_id.to_string(), name.to_string());
                        }
                        let args = function
                            .get("arguments")
                            .and_then(Value::as_str)
                            .and_then(|args| serde_json::from_str::<Value>(args).ok())
                            .unwrap_or_else(|| json!({}));
                        let mut function_call = json!({
                            "name": name,
                            "args": args,
                        });
                        if !call_id.trim().is_empty() {
                            function_call["id"] = Value::String(call_id.to_string());
                        }
                        parts.push(json!({ "functionCall": function_call }));
                    }
                }
                if !parts.is_empty() {
                    contents.push(json!({
                        "role": "model",
                        "parts": parts,
                    }));
                }
            }
            "tool" => {
                let mut parts = Vec::new();
                while index < items.len()
                    && items[index].get("role").and_then(Value::as_str) == Some("tool")
                {
                    let tool_item = &items[index];
                    let call_id = tool_item
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let name = tool_item
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| tool_names_by_call_id.get(call_id).cloned())
                        .unwrap_or_else(|| "tool_call".to_string());
                    let response = gemini_tool_response_from_message(tool_item);
                    let mut function_response = json!({
                        "name": name,
                        "response": response,
                    });
                    if !call_id.trim().is_empty() {
                        function_response["id"] = Value::String(call_id.to_string());
                    }
                    parts.push(json!({ "functionResponse": function_response }));
                    index += 1;
                }
                contents.push(json!({
                    "role": "user",
                    "parts": parts,
                }));
                continue;
            }
            _ => {
                if gemini_contextual_user_instruction_text(item).is_some() {
                    index += 1;
                    continue;
                }
                contents.push(json!({
                    "role":"user",
                    "parts":[{"text": gemini_message_text(item).unwrap_or_default()}],
                }));
            }
        }
        index += 1;
    }
    if contents.is_empty() {
        contents.push(json!({"role":"user","parts":[{"text":""}]}));
    }
    contents
}

fn gemini_tool_response_from_message(message: &Value) -> Value {
    let text = gemini_message_text(message).unwrap_or_default();
    serde_json::from_str::<Value>(&text).unwrap_or_else(|_| {
        json!({
            "output": text
        })
    })
}

fn gemini_message_text(message: &Value) -> Option<String> {
    match message.get("content") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => message
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn gemini_contextual_user_instruction_text(message: &Value) -> Option<String> {
    if message
        .get("role")
        .and_then(Value::as_str)
        .is_some_and(|role| role != "user")
    {
        return None;
    }
    let text = gemini_message_text(message)?;
    if gemini_is_contextual_user_fragment(&text)
        || text
            .split("\n\n")
            .filter(|fragment| !fragment.trim().is_empty())
            .all(gemini_is_contextual_user_fragment)
    {
        Some(text)
    } else {
        None
    }
}

fn gemini_is_contextual_user_fragment(text: &str) -> bool {
    let trimmed = text.trim_start();
    [
        "# AGENTS.md instructions for ",
        "<environment_context>",
        "<permissions instructions>",
        "<collaboration_mode>",
        "<skills_instructions>",
        "<plugins_instructions>",
        "<model_switch>",
        "<personality_spec>",
        "<realtime_conversation>",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

fn gemini_tool_from_openai_tool(tool: &Value) -> Option<Value> {
    let function = tool.get("function")?;
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("tool_call");
    let description = function
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let parameters = function
        .get("parameters")
        .map(sanitize_schema)
        .unwrap_or_else(|| json!({"type":"object","properties":{}}));
    Some(json!({
        "name": name,
        "description": description,
        "parameters": parameters,
    }))
}

fn gemini_builtin_tools_from_request(tools: &[Value]) -> Vec<Value> {
    let mut translated = Vec::new();
    if let Some(computer_use) = gemini_computer_use_tool(tools) {
        translated.push(json!({ "computerUse": computer_use }));
    }
    if tools.iter().any(gemini_is_code_execution_tool) {
        translated.push(json!({ "codeExecution": {} }));
    }
    if tools.iter().any(gemini_is_web_search_tool) {
        translated.push(json!({ "googleSearch": {} }));
    }
    if tools.iter().any(gemini_is_url_context_tool) {
        translated.push(json!({ "urlContext": {} }));
    }
    translated
}

fn gemini_computer_use_tool(tools: &[Value]) -> Option<Value> {
    let tool = tools
        .iter()
        .find(|tool| gemini_is_computer_use_tool(tool))?;
    let source = tool
        .get("computerUse")
        .or_else(|| tool.get("computer_use"))
        .unwrap_or(tool);
    let environment = source
        .get("environment")
        .and_then(Value::as_str)
        .filter(|environment| !environment.trim().is_empty())
        .unwrap_or("ENVIRONMENT_BROWSER");
    let mut computer_use = json!({
        "environment": environment,
    });
    if let Some(excluded) = source
        .get("excludedPredefinedFunctions")
        .or_else(|| source.get("excluded_predefined_functions"))
        .filter(|value| !value.is_null())
    {
        computer_use["excludedPredefinedFunctions"] = excluded.clone();
    }
    Some(computer_use)
}

fn gemini_is_computer_use_tool(tool: &Value) -> bool {
    let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or_default();
    matches!(
        tool_type,
        "computer" | "computer_use" | "computerUse" | "computer_use_preview"
    ) || tool_type.starts_with("computer_")
        || tool
            .as_object()
            .is_some_and(|object| object.contains_key("computerUse"))
}

fn gemini_is_code_execution_tool(tool: &Value) -> bool {
    let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or_default();
    matches!(
        tool_type,
        "code_interpreter" | "code_execution" | "codeExecution"
    ) || tool
        .as_object()
        .is_some_and(|object| object.contains_key("codeExecution"))
}

fn gemini_is_web_search_tool(tool: &Value) -> bool {
    let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or_default();
    tool_type == "web_search"
        || tool_type == "web_search_preview"
        || tool_type.starts_with("web_search_preview_")
}

fn gemini_is_url_context_tool(tool: &Value) -> bool {
    let tool_type = tool.get("type").and_then(Value::as_str).unwrap_or_default();
    tool_type == "web_fetch"
        || tool_type == "url_context"
        || tool_type == "urlContext"
        || tool_type == "web_fetch_preview"
        || tool_type.starts_with("web_fetch_preview_")
        || tool
            .as_object()
            .is_some_and(|object| object.contains_key("urlContext"))
}

fn gemini_insert_basic_generation_config(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
) {
    for (from, to) in [
        ("temperature", "temperature"),
        ("top_p", "topP"),
        ("max_tokens", "maxOutputTokens"),
    ] {
        if let Some(value) = obj.get(from).filter(|value| !value.is_null()) {
            generation_config.insert(to.to_string(), value.clone());
        }
    }
    if let Some(stop) = obj
        .get("stop")
        .or_else(|| obj.get("stop_sequences"))
        .or_else(|| obj.get("stopSequences"))
        .filter(|value| !value.is_null())
    {
        generation_config.insert("stopSequences".to_string(), stop.clone());
    }
}

fn gemini_thinking_config_from_request(
    obj: &serde_json::Map<String, Value>,
    model: &str,
) -> Option<Value> {
    let effort = obj
        .get("reasoning")
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(Value::as_str)
        .unwrap_or("high")
        .to_ascii_lowercase();
    if effort == "none" || effort == "minimal" {
        return Some(json!({
            "includeThoughts": false,
            "thinkingBudget": 0,
        }));
    }
    if gemini_model_uses_thinking_level(model) {
        let level = match effort.as_str() {
            "low" => "LOW",
            "medium" => "MEDIUM",
            _ => "HIGH",
        };
        return Some(json!({
            "includeThoughts": true,
            "thinkingLevel": level,
        }));
    }
    let budget = match effort.as_str() {
        "low" => 1024,
        "medium" | "high" => 8192,
        "xhigh" => 24576,
        _ => 8192,
    };
    Some(json!({
        "includeThoughts": true,
        "thinkingBudget": budget,
    }))
}

fn gemini_insert_extended_generation_config(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
) {
    for (from, to) in [
        ("top_k", "topK"),
        ("topK", "topK"),
        ("candidate_count", "candidateCount"),
        ("candidateCount", "candidateCount"),
        ("seed", "seed"),
        ("presence_penalty", "presencePenalty"),
        ("presencePenalty", "presencePenalty"),
        ("frequency_penalty", "frequencyPenalty"),
        ("frequencyPenalty", "frequencyPenalty"),
        ("response_mime_type", "responseMimeType"),
        ("responseMimeType", "responseMimeType"),
        ("response_schema", "responseSchema"),
        ("responseSchema", "responseSchema"),
        ("response_json_schema", "responseJsonSchema"),
        ("responseJsonSchema", "responseJsonSchema"),
        ("response_modalities", "responseModalities"),
        ("responseModalities", "responseModalities"),
        ("media_resolution", "mediaResolution"),
        ("mediaResolution", "mediaResolution"),
        ("audio_timestamp", "audioTimestamp"),
        ("audioTimestamp", "audioTimestamp"),
        ("speech_config", "speechConfig"),
        ("speechConfig", "speechConfig"),
    ] {
        if let Some(value) = obj.get(from).filter(|value| !value.is_null()) {
            generation_config.insert(to.to_string(), value.clone());
        }
    }
}

fn gemini_apply_text_format(
    obj: &serde_json::Map<String, Value>,
    generation_config: &mut serde_json::Map<String, Value>,
) {
    let Some(text) = obj.get("text").and_then(Value::as_object) else {
        return;
    };
    let Some(format) = text.get("format").and_then(Value::as_object) else {
        return;
    };
    let format_type = format
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match format_type {
        "json_object" => {
            generation_config.insert(
                "responseMimeType".to_string(),
                Value::String("application/json".to_string()),
            );
        }
        "json_schema" => {
            generation_config.insert(
                "responseMimeType".to_string(),
                Value::String("application/json".to_string()),
            );
            if let Some(schema) = format.get("schema").or_else(|| format.get("json_schema")) {
                generation_config.insert("responseJsonSchema".to_string(), schema.clone());
            }
        }
        _ => {}
    }
}

fn gemini_model_uses_thinking_level(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    model.contains("gemini-3") || model.contains("gemma-3") || model.contains("gemma-4")
}

fn gemini_tool_config_from_request(value: &Value) -> Option<Value> {
    let tool_choice = value.get("tool_choice")?;
    if tool_choice.as_str() == Some("auto") {
        return None;
    }
    if tool_choice.as_str() == Some("none") {
        return Some(json!({
            "functionCallingConfig": {
                "mode": "NONE",
            }
        }));
    }
    if tool_choice.as_str() == Some("required") {
        return Some(json!({
            "functionCallingConfig": {
                "mode": "ANY",
            }
        }));
    }
    let name = tool_choice
        .get("function")
        .and_then(|function| function.get("name"))
        .and_then(Value::as_str)
        .or_else(|| tool_choice.get("name").and_then(Value::as_str))?;
    Some(json!({
        "functionCallingConfig": {
            "mode": "ANY",
            "allowedFunctionNames": [name],
        }
    }))
}

fn sanitize_schema(schema: &Value) -> Value {
    match schema {
        Value::Array(values) => Value::Array(values.iter().map(sanitize_schema).collect()),
        Value::Object(map) => {
            let mut next = serde_json::Map::new();
            for (key, value) in map {
                if matches!(key.as_str(), "strict" | "$schema" | "additionalProperties") {
                    continue;
                }
                next.insert(key.clone(), sanitize_schema(value));
            }
            Value::Object(next)
        }
        _ => schema.clone(),
    }
}

fn gemini_normalized_response_value(value: &Value) -> Cow<'_, Value> {
    let Some(response) = value.get("response") else {
        return Cow::Borrowed(value);
    };
    let Some(trace_id) = value.get("traceId").and_then(Value::as_str) else {
        return Cow::Borrowed(response);
    };
    let mut response = response.clone();
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "responseId".to_string(),
            Value::String(trace_id.to_string()),
        );
    }
    Cow::Owned(response)
}

fn gemini_responses_value_from_generate_value(value: &Value) -> Value {
    let response_id = value
        .get("responseId")
        .and_then(Value::as_str)
        .unwrap_or("gemini_resp_prodex");
    let model = value
        .get("modelVersion")
        .and_then(Value::as_str)
        .unwrap_or("gemini-2.5-pro");
    let parts = value
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut output = Vec::new();
    let mut text = String::new();
    let mut content_items = Vec::new();
    for (index, part) in parts.into_iter().enumerate() {
        if let Some(part_text) = part.get("text").and_then(Value::as_str).filter(|_| {
            !part
                .get("thought")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        }) {
            text.push_str(part_text);
        }
        if let Some(part_text) = gemini_text_from_special_part(&part) {
            content_items.push(json!({
                "type": "output_text",
                "text": part_text,
            }));
        }
        if let Some(content_item) = gemini_media_content_item_from_part(&part) {
            content_items.push(content_item);
        }
        if let Some(image_generation) =
            gemini_image_generation_call_item_from_part(response_id, index, &part)
        {
            output.push(image_generation);
        }
        if let Some(function_call) = part.get("functionCall") {
            output.push(gemini_response_tool_call_item(&part, function_call));
        }
    }
    if !text.is_empty() {
        let mut content = vec![json!({
            "type":"output_text",
            "text": text,
        })];
        content.extend(content_items);
        output.insert(
            0,
            json!({
                "type":"message",
                "role":"assistant",
                "content": content,
            }),
        );
    } else if !content_items.is_empty() {
        output.insert(
            0,
            json!({
                "type":"message",
                "role":"assistant",
                "content": content_items,
            }),
        );
    }
    if let Some(grounding_call) = gemini_web_search_call_from_grounding(value, response_id) {
        output.push(grounding_call);
    }
    if let Some(citations) = gemini_citation_text(value) {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": citations,
            }],
        }));
    }
    let has_visible_output = !output.is_empty();
    let mut response = json!({
        "id": response_id,
        "object": "response",
        "model": model,
        "output": output,
        "usage": value.get("usageMetadata").and_then(gemini_responses_usage).unwrap_or_else(|| json!({})),
        "metadata": gemini_response_metadata(value).unwrap_or_else(|| json!({})),
    });
    if let Some(status) = gemini_response_status(value, has_visible_output) {
        match status {
            GeminiResponseStatus::Failed { code, message } => {
                response["status"] = Value::String("failed".to_string());
                response["error"] = json!({
                    "code": code,
                    "message": message,
                });
            }
            GeminiResponseStatus::Incomplete { reason, message } => {
                response["status"] = Value::String("incomplete".to_string());
                response["incomplete_details"] = json!({
                    "reason": reason,
                    "message": message,
                });
            }
        }
    }
    response
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum GeminiResponseStatus {
    Failed { code: String, message: String },
    Incomplete { reason: String, message: String },
}

fn gemini_response_status(value: &Value, has_visible_output: bool) -> Option<GeminiResponseStatus> {
    if let Some((code, message)) = gemini_prompt_feedback_failure(value) {
        return Some(GeminiResponseStatus::Failed { code, message });
    }
    if let Some(reason) = gemini_finish_reason(value) {
        if let Some((reason, message)) = gemini_finish_reason_incomplete(&reason) {
            return Some(GeminiResponseStatus::Incomplete { reason, message });
        }
        if let Some((code, message)) = gemini_finish_reason_failure(&reason) {
            return Some(GeminiResponseStatus::Failed { code, message });
        }
    }
    if !has_visible_output {
        let suffix = gemini_finish_reason(value)
            .map(|reason| format!(" finishReason={reason}"))
            .unwrap_or_default();
        return Some(GeminiResponseStatus::Failed {
            code: "gemini_empty_response".to_string(),
            message: format!("Gemini returned no visible response content.{suffix}"),
        });
    }
    None
}

fn gemini_prompt_feedback_failure(value: &Value) -> Option<(String, String)> {
    let feedback = value.get("promptFeedback")?;
    let reason = feedback
        .get("blockReason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.trim().is_empty())?;
    Some((
        "gemini_prompt_blocked".to_string(),
        format!("Gemini blocked the prompt: {reason}"),
    ))
}

fn gemini_finish_reason(value: &Value) -> Option<String> {
    value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("finishReason"))
        .and_then(Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .map(str::to_string)
}

fn gemini_finish_reason_failure(reason: &str) -> Option<(String, String)> {
    let code = match reason {
        "MALFORMED_FUNCTION_CALL" => "gemini_malformed_function_call",
        "UNEXPECTED_TOOL_CALL" => "gemini_unexpected_tool_call",
        "OTHER" => "gemini_finish_other",
        "NO_IMAGE" => "gemini_no_image",
        "SAFETY"
        | "RECITATION"
        | "LANGUAGE"
        | "BLOCKLIST"
        | "PROHIBITED_CONTENT"
        | "SPII"
        | "IMAGE_SAFETY"
        | "IMAGE_PROHIBITED_CONTENT" => "invalid_prompt",
        _ => return None,
    };
    Some((
        code.to_string(),
        format!("Gemini ended the stream with finishReason={reason}"),
    ))
}

fn gemini_finish_reason_incomplete(reason: &str) -> Option<(String, String)> {
    match reason {
        "MAX_TOKENS" => Some((
            "max_output_tokens".to_string(),
            "Gemini stopped because it reached the maximum output token limit.".to_string(),
        )),
        _ => None,
    }
}

fn gemini_citation_text(value: &Value) -> Option<String> {
    gemini_finish_reason(value)?;
    let citations = value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("citationMetadata"))
        .and_then(|metadata| metadata.get("citations"))
        .and_then(Value::as_array)?;
    let mut lines = citations
        .iter()
        .filter_map(|citation| {
            let uri = citation
                .get("uri")
                .and_then(Value::as_str)
                .filter(|uri| !uri.trim().is_empty())?;
            let title = citation
                .get("title")
                .and_then(Value::as_str)
                .filter(|title| !title.trim().is_empty());
            Some(match title {
                Some(title) => format!("({title}) {uri}"),
                None => uri.to_string(),
            })
        })
        .collect::<Vec<_>>();
    lines.sort();
    lines.dedup();
    (!lines.is_empty()).then(|| format!("Citations:\n{}", lines.join("\n")))
}

fn gemini_web_search_call_from_grounding(value: &Value, response_id: &str) -> Option<Value> {
    let candidate = value.get("candidates")?.as_array()?.first()?;
    let mut sources = Vec::new();
    let grounding_metadata = candidate.get("groundingMetadata");
    if let Some(chunks) = grounding_metadata
        .and_then(|metadata| metadata.get("groundingChunks"))
        .and_then(Value::as_array)
    {
        for chunk in chunks {
            for source_kind in ["web", "retrievedContext"] {
                if let Some(source) = chunk
                    .get(source_kind)
                    .and_then(gemini_url_source_from_metadata)
                {
                    gemini_push_unique_url_source(&mut sources, source);
                }
            }
        }
    }
    if let Some(citations) = candidate
        .get("citationMetadata")
        .and_then(|metadata| {
            metadata
                .get("citations")
                .or_else(|| metadata.get("citationSources"))
        })
        .and_then(Value::as_array)
    {
        for citation in citations {
            if let Some(source) = gemini_url_source_from_metadata(citation) {
                gemini_push_unique_url_source(&mut sources, source);
            }
        }
    }
    if let Some(url_metadata) = candidate
        .get("urlContextMetadata")
        .and_then(|metadata| {
            metadata
                .get("urlMetadata")
                .or_else(|| metadata.get("url_metadata"))
        })
        .and_then(Value::as_array)
    {
        for entry in url_metadata {
            if let Some(source) = gemini_url_source_from_metadata(entry) {
                gemini_push_unique_url_source(&mut sources, source);
            }
        }
    }
    let queries = grounding_metadata
        .and_then(|metadata| metadata.get("webSearchQueries"))
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    if sources.is_empty() && queries.is_empty() {
        return None;
    }
    let action = if queries.is_empty() {
        let first_url = sources
            .first()
            .and_then(|source| source.get("url"))
            .and_then(Value::as_str)?;
        json!({
            "type": "open_page",
            "url": first_url,
            "sources": sources,
        })
    } else {
        json!({
            "type": "search",
            "queries": queries,
            "sources": sources,
        })
    };
    Some(json!({
        "type": "web_search_call",
        "id": format!("ws_{response_id}"),
        "status": "completed",
        "action": action,
    }))
}

fn gemini_url_source_from_metadata(value: &Value) -> Option<Value> {
    let uri = ["uri", "url", "retrievedUrl", "retrieved_url"]
        .into_iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .filter(|uri| !uri.trim().is_empty())?;
    let mut source = json!({
        "type": "url",
        "url": uri,
    });
    if let Some(title) = value
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
    {
        source["title"] = Value::String(title.to_string());
    }
    if let Some(status) = value
        .get("urlRetrievalStatus")
        .or_else(|| value.get("url_retrieval_status"))
        .or_else(|| value.get("status"))
        .filter(|status| !status.is_null())
    {
        source["status"] = status.clone();
    }
    Some(source)
}

fn gemini_push_unique_url_source(sources: &mut Vec<Value>, source: Value) {
    let source_url = source.get("url").and_then(Value::as_str);
    if sources
        .iter()
        .any(|existing| existing.get("url").and_then(Value::as_str) == source_url)
    {
        return;
    }
    sources.push(source);
}

fn gemini_media_content_item_from_part(part: &Value) -> Option<Value> {
    if let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data")) {
        let mime_type = inline_data
            .get("mimeType")
            .or_else(|| inline_data.get("mime_type"))
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream");
        let data = inline_data.get("data").and_then(Value::as_str)?;
        if mime_type.starts_with("image/") {
            return Some(json!({
                "type": "input_image",
                "image_url": format!("data:{mime_type};base64,{data}"),
            }));
        }
        return Some(json!({
            "type": "output_text",
            "text": format!(
                "Gemini returned inline {mime_type} media ({} base64 characters).",
                data.len()
            ),
        }));
    }
    if let Some(file_data) = part.get("fileData").or_else(|| part.get("file_data")) {
        let file_uri = file_data
            .get("fileUri")
            .or_else(|| file_data.get("file_uri"))
            .and_then(Value::as_str)?;
        let mime_type = file_data
            .get("mimeType")
            .or_else(|| file_data.get("mime_type"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| gemini_mime_type_for_uri(file_uri));
        if mime_type.starts_with("image/") {
            return Some(json!({
                "type": "input_image",
                "image_url": file_uri,
            }));
        }
        return Some(json!({
            "type": "output_text",
            "text": format!("Gemini returned {mime_type} media: {file_uri}"),
        }));
    }
    let text = part.get("text").and_then(Value::as_str)?;
    let (mime_type, data) = gemini_data_url_parts(text)?;
    if mime_type.starts_with("image/") {
        Some(json!({
            "type": "input_image",
            "image_url": format!("data:{mime_type};base64,{data}"),
        }))
    } else {
        Some(json!({
            "type": "output_text",
            "text": format!(
                "Gemini returned inline {mime_type} media ({} base64 characters).",
                data.len()
            ),
        }))
    }
}

fn gemini_text_from_special_part(part: &Value) -> Option<String> {
    if let Some(executable_code) = part.get("executableCode") {
        let language = executable_code
            .get("language")
            .and_then(Value::as_str)
            .unwrap_or("text");
        let code = executable_code
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if code.trim().is_empty() {
            return None;
        }
        return Some(format!(
            "Gemini executable code ({language}):\n```{language}\n{code}\n```"
        ));
    }
    if let Some(result) = part.get("codeExecutionResult") {
        let outcome = result
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("OUTCOME_UNSPECIFIED");
        let output = result
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Some(format!(
            "Gemini code execution result ({outcome}):\n```text\n{output}\n```"
        ));
    }
    if let Some(video_metadata) = part.get("videoMetadata") {
        let metadata = serde_json::to_string(video_metadata).unwrap_or_else(|_| "{}".to_string());
        return Some(format!("Gemini video metadata: {metadata}"));
    }
    None
}

fn gemini_image_generation_call_item_from_part(
    response_id: &str,
    index: usize,
    part: &Value,
) -> Option<Value> {
    let inline_data = part.get("inlineData").or_else(|| part.get("inline_data"))?;
    let mime_type = inline_data
        .get("mimeType")
        .or_else(|| inline_data.get("mime_type"))
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream");
    if !mime_type.starts_with("image/") {
        return None;
    }
    let data = inline_data.get("data").and_then(Value::as_str)?;
    Some(json!({
        "type": "image_generation_call",
        "id": format!("ig_{response_id}_{index}"),
        "status": "completed",
        "result": data,
    }))
}

fn gemini_data_url_parts(text: &str) -> Option<(&str, &str)> {
    let data = text.strip_prefix("data:")?;
    let (meta, body) = data.split_once(',')?;
    let meta = meta.strip_suffix(";base64")?;
    Some((meta, body))
}

fn gemini_mime_type_for_uri(uri: &str) -> &str {
    match uri
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn gemini_response_tool_call_item(part: &Value, function_call: &Value) -> Value {
    let call_id = function_call
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("call_1");
    let flat_name = function_call
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("tool_call");
    let args_value = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if flat_name == "tool_search" {
        return json!({
            "type": "tool_search_call",
            "call_id": call_id,
            "execution": "client",
            "arguments": args_value,
        });
    }
    if let Some(item) = gemini_custom_tool_call_item(call_id, flat_name, &args_value) {
        return item;
    }
    let args = serde_json::to_string(&args_value).unwrap_or_else(|_| "{}".to_string());
    let (namespace, name) = gemini_split_flat_namespace_tool_name(flat_name);
    let mut item = json!({
        "type": "function_call",
        "call_id": call_id,
        "name": name,
        "arguments": gemini_rtk_wrapped_tool_arguments(flat_name, &args),
    });
    if let Some(namespace) = namespace {
        item["namespace"] = Value::String(namespace);
    }
    if let Some(signature) = part
        .get("thoughtSignature")
        .and_then(Value::as_str)
        .or_else(|| {
            function_call
                .get("thoughtSignature")
                .and_then(Value::as_str)
        })
    {
        item["gemini_thought_signature"] = Value::String(signature.to_string());
    }
    item
}

fn gemini_custom_tool_call_item(
    call_id: &str,
    flat_name: &str,
    args_value: &Value,
) -> Option<Value> {
    if flat_name != "apply_patch" {
        return None;
    }
    Some(json!({
        "type": "custom_tool_call",
        "call_id": call_id,
        "name": flat_name,
        "input": gemini_custom_apply_patch_input(args_value),
    }))
}

fn gemini_custom_apply_patch_input(args_value: &Value) -> String {
    if let Some(input) = gemini_explicit_apply_patch_input(args_value) {
        return gemini_normalize_apply_patch_input(input).unwrap_or_else(|| input.to_string());
    }
    if let Some(input) = gemini_edit_args_to_apply_patch(args_value) {
        return input;
    }
    if let Some(input) = gemini_content_apply_patch_input(args_value) {
        return gemini_normalize_apply_patch_input(input).unwrap_or_else(|| input.to_string());
    }
    let input = args_value.to_string();
    gemini_normalize_apply_patch_input(&input).unwrap_or(input)
}

fn gemini_explicit_apply_patch_input(args_value: &Value) -> Option<&str> {
    match args_value {
        Value::String(input) => Some(input),
        Value::Object(object) => ["input", "patch", "diff", "text"]
            .iter()
            .find_map(|key| object.get(*key).and_then(Value::as_str)),
        _ => None,
    }
}

fn gemini_content_apply_patch_input(args_value: &Value) -> Option<&str> {
    args_value
        .as_object()?
        .get("content")
        .and_then(Value::as_str)
}

fn gemini_normalize_apply_patch_input(input: &str) -> Option<String> {
    gemini_extract_apply_patch_block(input).or_else(|| gemini_unified_diff_to_apply_patch(input))
}

fn gemini_extract_apply_patch_block(input: &str) -> Option<String> {
    let normalized = gemini_normalized_newlines(input);
    let lines = normalized.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with("*** Begin Patch"))?;
    let end = lines[start..]
        .iter()
        .position(|line| line.trim_start().starts_with("*** End Patch"))?
        + start;
    Some(gemini_repair_apply_patch_block(
        &lines[start..=end].join("\n"),
    ))
}

fn gemini_repair_apply_patch_block(input: &str) -> String {
    let normalized = gemini_normalized_newlines(input);
    let mut output = Vec::new();
    let mut in_add_file = false;
    for line in normalized.lines() {
        if line.starts_with("*** Add File: ") {
            in_add_file = true;
            output.push(line.to_string());
            continue;
        }
        if line.starts_with("*** ") {
            in_add_file = false;
            output.push(line.to_string());
            continue;
        }
        if in_add_file && !line.starts_with('+') {
            output.push(format!("+{line}"));
            continue;
        }
        output.push(line.to_string());
    }
    output.join("\n")
}

fn gemini_edit_args_to_apply_patch(args_value: &Value) -> Option<String> {
    let object = args_value.as_object()?;
    let path = ["file_path", "path", "filename"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .filter(|path| !path.trim().is_empty())?
        .trim();
    let old_string = ["old_string", "old", "search", "find"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str));
    let new_string = ["new_string", "new", "replace", "replacement", "content"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str));
    match (old_string, new_string) {
        (Some(""), Some(new_string)) => Some(gemini_add_file_patch(path, new_string)),
        (Some(old_string), Some(new_string)) => {
            Some(gemini_update_file_patch(path, old_string, new_string))
        }
        (None, Some(new_string)) => Some(gemini_add_file_patch(path, new_string)),
        _ => None,
    }
}

fn gemini_unified_diff_to_apply_patch(input: &str) -> Option<String> {
    let normalized = gemini_normalized_newlines(input);
    let lines = normalized.lines().collect::<Vec<_>>();
    let mut output = vec!["*** Begin Patch".to_string()];
    let mut converted_files = 0usize;
    let mut index = 0usize;
    while index < lines.len() {
        if lines[index].starts_with("diff --git ") {
            let end = gemini_next_diff_section(&lines, index + 1);
            converted_files += gemini_convert_diff_section(&lines[index..end], &mut output);
            index = end;
            continue;
        }
        if gemini_is_unified_file_header(&lines, index) {
            let end = gemini_next_header_or_diff_section(&lines, index + 2);
            converted_files += gemini_convert_diff_section(&lines[index..end], &mut output);
            index = end;
            continue;
        }
        index += 1;
    }
    if converted_files == 0 {
        return None;
    }
    output.push("*** End Patch".to_string());
    Some(output.join("\n"))
}

fn gemini_convert_diff_section(lines: &[&str], output: &mut Vec<String>) -> usize {
    let Some(header_index) =
        (0..lines.len()).find(|index| gemini_is_unified_file_header(lines, *index))
    else {
        return 0;
    };
    let old_path = gemini_unified_diff_path(lines[header_index]);
    let new_path = gemini_unified_diff_path(lines[header_index + 1]);
    let body = &lines[(header_index + 2)..];
    match (old_path, new_path) {
        (None, Some(path)) => gemini_convert_add_file(&path, body, output),
        (Some(path), None) => {
            output.push(format!("*** Delete File: {path}"));
            1
        }
        (Some(old_path), Some(new_path)) => {
            gemini_convert_update_file(&old_path, &new_path, body, output)
        }
        (None, None) => 0,
    }
}

fn gemini_convert_add_file(path: &str, lines: &[&str], output: &mut Vec<String>) -> usize {
    let hunk_start = output.len();
    output.push(format!("*** Add File: {path}"));
    let mut saw_added_line = false;
    for line in lines {
        if line.starts_with('+') && !line.starts_with("+++") {
            output.push((*line).to_string());
            saw_added_line = true;
        }
    }
    if saw_added_line {
        1
    } else {
        output.truncate(hunk_start);
        0
    }
}

fn gemini_convert_update_file(
    old_path: &str,
    new_path: &str,
    lines: &[&str],
    output: &mut Vec<String>,
) -> usize {
    let hunk_start = output.len();
    output.push(format!("*** Update File: {old_path}"));
    if old_path != new_path {
        output.push(format!("*** Move to: {new_path}"));
    }
    let mut saw_change = false;
    let mut saw_hunk = false;
    for line in lines {
        if line.starts_with("@@") {
            output.push(line.to_string());
            saw_hunk = true;
        } else if saw_hunk && gemini_is_apply_patch_change_line(line) {
            output.push((*line).to_string());
            saw_change |= line.starts_with('+') || line.starts_with('-');
        } else if saw_hunk && *line == "\\ No newline at end of file" {
            output.push("*** End of File".to_string());
        }
    }
    if saw_change {
        1
    } else {
        output.truncate(hunk_start);
        0
    }
}

fn gemini_add_file_patch(path: &str, new_string: &str) -> String {
    let mut lines = vec![
        "*** Begin Patch".to_string(),
        format!("*** Add File: {path}"),
    ];
    if new_string.is_empty() {
        lines.push("+".to_string());
    } else {
        lines.extend(
            gemini_normalized_newlines(new_string)
                .lines()
                .map(|line| format!("+{line}")),
        );
    }
    lines.push("*** End Patch".to_string());
    lines.join("\n")
}

fn gemini_update_file_patch(path: &str, old_string: &str, new_string: &str) -> String {
    let mut lines = vec![
        "*** Begin Patch".to_string(),
        format!("*** Update File: {path}"),
        "@@".to_string(),
    ];
    lines.extend(
        gemini_normalized_newlines(old_string)
            .lines()
            .map(|line| format!("-{line}")),
    );
    lines.extend(
        gemini_normalized_newlines(new_string)
            .lines()
            .map(|line| format!("+{line}")),
    );
    lines.push("*** End Patch".to_string());
    lines.join("\n")
}

fn gemini_normalized_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

fn gemini_next_diff_section(lines: &[&str], start: usize) -> usize {
    (start..lines.len())
        .find(|index| lines[*index].starts_with("diff --git "))
        .unwrap_or(lines.len())
}

fn gemini_next_header_or_diff_section(lines: &[&str], start: usize) -> usize {
    (start..lines.len())
        .find(|index| {
            lines[*index].starts_with("diff --git ") || gemini_is_unified_file_header(lines, *index)
        })
        .unwrap_or(lines.len())
}

fn gemini_is_unified_file_header(lines: &[&str], index: usize) -> bool {
    index + 1 < lines.len()
        && lines[index].starts_with("--- ")
        && lines[index + 1].starts_with("+++ ")
}

fn gemini_unified_diff_path(line: &str) -> Option<String> {
    let path = line
        .strip_prefix("--- ")
        .or_else(|| line.strip_prefix("+++ "))?
        .trim_matches('"');
    if matches!(path, "/dev/null") {
        return None;
    }
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .trim()
        .to_string()
        .into()
}

fn gemini_is_apply_patch_change_line(line: &str) -> bool {
    line.starts_with('+') || line.starts_with('-') || line.starts_with(' ')
}

fn gemini_split_flat_namespace_tool_name(name: &str) -> (Option<String>, String) {
    if let Some((namespace, tool_name)) = name.rsplit_once("--")
        && !namespace.trim().is_empty()
        && !tool_name.trim().is_empty()
    {
        return (Some(namespace.to_string()), tool_name.to_string());
    }
    let Some(rest) = name.strip_prefix("mcp__") else {
        return (None, name.to_string());
    };
    let Some(index) = rest.rfind("__") else {
        return (None, name.to_string());
    };
    let namespace = format!("mcp__{}", &rest[..index]);
    let tool_name = rest[index + 2..].to_string();
    if tool_name.trim().is_empty() {
        return (None, name.to_string());
    }
    (Some(namespace), tool_name)
}

fn gemini_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    if !matches!(name, "shell" | "exec_command") {
        return arguments.to_string();
    }
    let Ok(mut value) = serde_json::from_str::<Value>(arguments) else {
        return arguments.to_string();
    };
    let Some(object) = value.as_object_mut() else {
        return arguments.to_string();
    };
    for key in ["cmd", "command"] {
        let Some(wrapped) = object
            .get(key)
            .and_then(Value::as_str)
            .and_then(gemini_rtk_wrapped_shell_command)
        else {
            continue;
        };
        object.insert(key.to_string(), Value::String(wrapped));
        return serde_json::to_string(&value).unwrap_or_else(|_| arguments.to_string());
    }
    arguments.to_string()
}

fn gemini_rtk_wrapped_shell_command(command: &str) -> Option<String> {
    let insert_at = gemini_rtk_insert_index(command)?;
    Some(format!(
        "{}rtk {}",
        &command[..insert_at],
        &command[insert_at..]
    ))
}

fn gemini_rtk_insert_index(command: &str) -> Option<usize> {
    let mut segment_start = 0;
    let mut index = 0;
    while index <= command.len() {
        if index == command.len() {
            return gemini_rtk_insert_index_in_segment(&command[segment_start..])
                .map(|offset| segment_start + offset);
        }
        let rest = &command[index..];
        let separator_len = if rest.starts_with("&&") || rest.starts_with("||") {
            2
        } else if rest.starts_with(';') || rest.starts_with('|') || rest.starts_with('\n') {
            1
        } else {
            0
        };
        if separator_len > 0 {
            if let Some(offset) = gemini_rtk_insert_index_in_segment(&command[segment_start..index])
            {
                return Some(segment_start + offset);
            }
            index += separator_len;
            segment_start = index;
            continue;
        }
        let Some((next_index, _)) = command[index..]
            .char_indices()
            .nth(1)
            .map(|(offset, ch)| (index + offset, ch))
        else {
            index = command.len();
            continue;
        };
        index = next_index;
    }
    None
}

fn gemini_rtk_insert_index_in_segment(segment: &str) -> Option<usize> {
    let mut offset = gemini_skip_shell_whitespace(segment, 0);
    if gemini_shell_token_at(segment, offset).is_some_and(|token| token == "rtk") {
        return None;
    }
    while let Some(token) = gemini_shell_token_at(segment, offset) {
        if !gemini_is_env_assignment(token) {
            break;
        }
        offset += token.len();
        offset = gemini_skip_shell_whitespace(segment, offset);
    }
    let token = gemini_shell_token_at(segment, offset)?;
    let command_name = token.rsplit('/').next().unwrap_or(token);
    let spec = GEMINI_RTK_NOISY_SHELL_COMMANDS
        .iter()
        .find(|spec| spec.command == command_name)?;
    if spec.subcommands.is_empty()
        || gemini_shell_segment_has_subcommand(&segment[offset + token.len()..], spec)
    {
        Some(offset)
    } else {
        None
    }
}

fn gemini_skip_shell_whitespace(segment: &str, mut offset: usize) -> usize {
    while let Some(ch) = segment[offset..].chars().next()
        && ch.is_whitespace()
    {
        offset += ch.len_utf8();
    }
    offset
}

fn gemini_shell_token_at(segment: &str, offset: usize) -> Option<&str> {
    if offset >= segment.len() {
        return None;
    }
    let end = segment[offset..]
        .char_indices()
        .find_map(|(relative, ch)| ch.is_whitespace().then_some(offset + relative))
        .unwrap_or(segment.len());
    (end > offset).then(|| &segment[offset..end])
}

fn gemini_is_env_assignment(token: &str) -> bool {
    let Some((name, value)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && !value.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
}

fn gemini_shell_segment_has_subcommand(segment: &str, spec: &GeminiRtkNoisyShellCommand) -> bool {
    segment.split_whitespace().any(|token| {
        let token = token.trim_matches(|ch: char| matches!(ch, '"' | '\'' | '(' | ')' | '{' | '}'));
        spec.subcommands.contains(&token)
    })
}

struct GeminiRtkNoisyShellCommand {
    command: &'static str,
    subcommands: &'static [&'static str],
}

const GEMINI_RTK_NOISY_SHELL_COMMANDS: &[GeminiRtkNoisyShellCommand] = &[
    GeminiRtkNoisyShellCommand {
        command: "git",
        subcommands: &["diff", "show", "log", "status", "grep", "blame"],
    },
    GeminiRtkNoisyShellCommand {
        command: "cargo",
        subcommands: &["test", "build", "check", "clippy", "bench", "run"],
    },
    GeminiRtkNoisyShellCommand {
        command: "npm",
        subcommands: &["test", "run", "build", "install", "ci", "update", "audit"],
    },
    GeminiRtkNoisyShellCommand {
        command: "yarn",
        subcommands: &["test", "run", "build", "install", "add", "upgrade"],
    },
    GeminiRtkNoisyShellCommand {
        command: "pnpm",
        subcommands: &["test", "run", "build", "install", "add", "update"],
    },
    GeminiRtkNoisyShellCommand {
        command: "bun",
        subcommands: &["test", "run", "build", "install", "add"],
    },
    GeminiRtkNoisyShellCommand {
        command: "pytest",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "go",
        subcommands: &["test", "build", "vet"],
    },
    GeminiRtkNoisyShellCommand {
        command: "docker",
        subcommands: &["build", "compose", "logs", "pull", "push", "run"],
    },
    GeminiRtkNoisyShellCommand {
        command: "kubectl",
        subcommands: &["logs", "describe", "get", "events", "top"],
    },
    GeminiRtkNoisyShellCommand {
        command: "rg",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "find",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "ls",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "tree",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "claw-compactor",
        subcommands: &["benchmark", "compact", "summarize"],
    },
    GeminiRtkNoisyShellCommand {
        command: "prodex-claw-compactor-auto",
        subcommands: &[],
    },
];

fn gemini_responses_usage(usage: &Value) -> Option<Value> {
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| input_tokens.saturating_add(output_tokens));
    let cached_tokens = usage
        .get("cachedContentTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning_tokens = usage
        .get("thoughtsTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let tool_tokens = usage
        .get("toolUsePromptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some(json!({
        "input_tokens": input_tokens,
        "input_tokens_details": {
            "cached_tokens": cached_tokens,
            "tool_tokens": tool_tokens,
        },
        "output_tokens": output_tokens,
        "output_tokens_details": {
            "reasoning_tokens": reasoning_tokens,
        },
        "total_tokens": total_tokens,
    }))
}

fn gemini_response_metadata(value: &Value) -> Option<Value> {
    let mut gemini = serde_json::Map::new();
    for key in ["promptFeedback", "usageMetadata"] {
        if let Some(field) = value.get(key).filter(|field| !field.is_null()) {
            gemini.insert(key.to_string(), field.clone());
        }
    }
    if let Some(candidate) = value
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
    {
        for key in [
            "finishReason",
            "finishMessage",
            "safetyRatings",
            "citationMetadata",
            "groundingMetadata",
            "urlContextMetadata",
            "avgLogprobs",
            "logprobsResult",
        ] {
            if let Some(field) = candidate.get(key).filter(|field| !field.is_null()) {
                gemini.insert(key.to_string(), field.clone());
            }
        }
    }
    if gemini.is_empty() {
        return None;
    }
    Some(json!({
        "gemini": Value::Object(gemini),
    }))
}
