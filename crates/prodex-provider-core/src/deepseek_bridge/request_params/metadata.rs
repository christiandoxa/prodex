//! DeepSeek response-format metadata and degraded JSON-mode notes.

use prodex_mojo_core::rich::{DeepSeekRequestPolicyOperation, deepseek_request_policy};

use super::super::request_policy::{detail, plan_value};

struct MojoMetadataRequest<'a> {
    existing: serde_json::Map<String, serde_json::Value>,
    provider_label: &'a str,
    provider_key: &'a str,
    client_metadata: Option<&'a serde_json::Value>,
    prompt_cache_key: Option<&'a str>,
    prompt_cache_retention: Option<&'a str>,
    degraded_from: Option<&'a str>,
    tool_choice: Option<&'a serde_json::Value>,
    thinking_enabled: bool,
}

fn mojo_metadata_value(
    request: MojoMetadataRequest<'_>,
) -> Result<Option<serde_json::Value>, String> {
    let MojoMetadataRequest {
        existing,
        provider_label,
        provider_key,
        client_metadata,
        prompt_cache_key,
        prompt_cache_retention,
        degraded_from,
        tool_choice,
        thinking_enabled,
    } = request;
    let mut base = existing;
    let provider = base.remove(provider_key);
    let base = serde_json::to_string(&base)
        .map_err(|error| format!("{provider_label} metadata serialization failed: {error}"))?;
    let provider = provider
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| format!("{provider_label} metadata serialization failed: {error}"))?;
    let client_metadata = client_metadata
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| {
            format!("{provider_label} client_metadata serialization failed: {error}")
        })?;
    let tool_choice = tool_choice
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| format!("{provider_label} tool_choice serialization failed: {error}"))?;
    let degraded_reason = degraded_from.map(|_| {
        format!(
            "{provider_label} response_format supports json_object but not native JSON Schema enforcement"
        )
    });
    let omitted_reason = (thinking_enabled && tool_choice.is_some()).then(|| {
        format!(
            "{provider_label} thinking mode currently rejects explicit tool_choice on the OpenAI Chat route, so Prodex omits it while preserving translated function tools"
        )
    });
    let mut input =
        super::DeepSeekKernelInput::new(super::DeepSeekKernelOperation::RequestMetadata);
    input.extra = Some(&base);
    input.metadata = provider.as_deref();
    input.item = client_metadata.as_deref();
    input.content = prompt_cache_key;
    input.reasoning_content = prompt_cache_retention;
    input.response = degraded_from;
    input.error_message = degraded_reason.as_deref();
    input.name = Some(provider_key);
    input.tool_choice = tool_choice.as_deref();
    input.arguments = omitted_reason.as_deref();
    input.stream = thinking_enabled;
    let mapped = super::deepseek_provider_core_mojo_value(input).map_err(|error| {
        format!("{provider_label} response metadata normalization failed: {error}")
    })?;
    let object = mapped.as_object().ok_or_else(|| {
        format!("{provider_label} response metadata normalization returned a non-object")
    })?;
    Ok((!object.is_empty()).then_some(mapped))
}

pub fn deepseek_provider_core_response_format_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
) -> Result<Option<serde_json::Value>, String> {
    let (source, plan) = plan_value(
        value,
        DeepSeekRequestPolicyOperation::ResponseFormatShape,
        false,
    );
    match plan.tag {
        0 | 2 => Ok(None),
        1 => {
            let format_type = detail(&source, plan).ok_or_else(|| {
                format!("{provider_label} response_format could not be classified")
            })?;
            let mut input =
                super::DeepSeekKernelInput::new(super::DeepSeekKernelOperation::ResponseFormat);
            input.role = Some(&format_type);
            super::deepseek_provider_core_mojo_value(input)
                .map(Some)
                .map_err(|error| {
                    format!("{provider_label} response_format could not be normalized: {error}")
                })
        }
        3 => Err(format!(
            "{provider_label} response_format must include a type"
        )),
        4 => {
            let format_type = detail(&source, plan).unwrap_or_default();
            Err(format!(
                "{provider_label} response_format type \x60{format_type}\x60 is not supported"
            ))
        }
        _ => Err(format!(
            "{provider_label} response_format classification returned invalid output"
        )),
    }
}

pub fn deepseek_provider_core_response_metadata_from_responses_request(
    value: &serde_json::Value,
    provider_label: &str,
    provider_key: &str,
) -> Result<Option<serde_json::Value>, String> {
    let (source, plan) = plan_value(value, DeepSeekRequestPolicyOperation::MetadataShape, false);
    if plan.tag == 1 {
        return Err(format!(
            "{provider_label} request metadata must be an object"
        ));
    }

    let metadata = match value.get("metadata") {
        Some(metadata) => metadata
            .as_object()
            .cloned()
            .ok_or_else(|| format!("{provider_label} request metadata must be an object"))?,
        None => serde_json::Map::new(),
    };
    if metadata
        .get(provider_key)
        .is_some_and(|provider_metadata| !provider_metadata.is_object())
    {
        return Err(format!(
            "{provider_label} request metadata.{provider_key} must be an object"
        ));
    }

    match plan.tag {
        0 | 256 => {}
        2 => {
            return Err(format!(
                "{provider_label} client_metadata must be an object"
            ));
        }
        3 => {
            return Err(format!(
                "{provider_label} prompt_cache_key must be a string"
            ));
        }
        4 => {
            return Err(format!(
                "{provider_label} prompt_cache_retention must be a string"
            ));
        }
        _ => {
            return Err(format!(
                "{provider_label} request metadata classification returned invalid output"
            ));
        }
    }

    let client_metadata = value.get("client_metadata");
    let prompt_cache_key = value
        .get("prompt_cache_key")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let prompt_cache_retention = value
        .get("prompt_cache_retention")
        .and_then(serde_json::Value::as_str);
    let degraded_from = (plan.tag == 256).then(|| detail(&source, plan)).flatten();

    mojo_metadata_value(MojoMetadataRequest {
        existing: metadata,
        provider_label,
        provider_key,
        client_metadata,
        prompt_cache_key,
        prompt_cache_retention,
        degraded_from: degraded_from.as_deref(),
        tool_choice: None,
        thinking_enabled: false,
    })
}

pub fn deepseek_provider_core_note_thinking_tool_choice_omission(
    value: &serde_json::Value,
    thinking_enabled: bool,
    provider_label: &str,
    provider_key: &str,
    response_metadata: &mut Option<serde_json::Value>,
) {
    if !thinking_enabled {
        return;
    }
    let Some(tool_choice) = value.get("tool_choice") else {
        return;
    };
    let existing = match response_metadata.as_ref() {
        Some(serde_json::Value::Object(object)) => object.clone(),
        Some(_) => return,
        None => serde_json::Map::new(),
    };
    let mapped = mojo_metadata_value(MojoMetadataRequest {
        existing,
        provider_label,
        provider_key,
        client_metadata: None,
        prompt_cache_key: None,
        prompt_cache_retention: None,
        degraded_from: None,
        tool_choice: Some(tool_choice),
        thinking_enabled: true,
    });
    // ponytail: retain metadata on bounded ABI failure; report errors if this API gains a Result.
    if let Ok(mapped) = mapped {
        *response_metadata = mapped;
    }
}

pub fn deepseek_provider_core_ensure_json_prompt_instruction(
    messages: &mut Vec<serde_json::Value>,
) {
    let source = serde_json::to_string(messages).expect("DeepSeek JSON guidance input serializes");
    let plan = deepseek_request_policy(
        DeepSeekRequestPolicyOperation::JsonGuidance,
        &source,
        false,
        0,
    )
    .expect("Mojo DeepSeek JSON guidance policy returned invalid output");
    match plan.tag {
        1 => return,
        0 => {}
        _ => panic!("Mojo DeepSeek JSON guidance policy returned invalid tag"),
    }

    messages.insert(
        0,
        serde_json::json!({
            "role": "system",
            "content": "Respond with valid JSON only.",
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        deepseek_provider_core_ensure_json_prompt_instruction,
        deepseek_provider_core_note_thinking_tool_choice_omission,
        deepseek_provider_core_response_format_from_responses_request,
        deepseek_provider_core_response_metadata_from_responses_request,
    };
    use serde_json::json;

    #[test]
    fn empty_provider_metadata_survives_mojo_normalization() {
        let request = json!({"metadata": {"deepseek": {}}});
        assert_eq!(
            deepseek_provider_core_response_metadata_from_responses_request(
                &request, "DeepSeek", "deepseek"
            )
            .unwrap(),
            Some(json!({"deepseek": {}}))
        );

        let request = json!({
            "metadata": {"deepseek": {}},
            "response_format": {"type": "json_schema"}
        });
        let metadata = deepseek_provider_core_response_metadata_from_responses_request(
            &request, "DeepSeek", "deepseek",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            metadata["deepseek"]["degraded_response_format"]["from"],
            "json_schema"
        );
    }

    #[test]
    fn mojo_response_format_shape_preserves_supported_and_error_classes() {
        assert_eq!(
            deepseek_provider_core_response_format_from_responses_request(
                &json!({"response_format": {"type": "text"}}),
                "DeepSeek",
            )
            .unwrap(),
            None
        );
        assert_eq!(
            deepseek_provider_core_response_format_from_responses_request(
                &json!({"text": {"format": {"type": "json"}}}),
                "DeepSeek",
            )
            .unwrap(),
            Some(json!({"type": "json_object"}))
        );
        assert_eq!(
            deepseek_provider_core_response_format_from_responses_request(
                &json!({"response_format": {}}),
                "DeepSeek",
            )
            .unwrap_err(),
            "DeepSeek response_format must include a type"
        );
        assert_eq!(
            deepseek_provider_core_response_format_from_responses_request(
                &json!({"response_format": {"type": "xml"}}),
                "DeepSeek",
            )
            .unwrap_err(),
            "DeepSeek response_format type `xml` is not supported"
        );
    }

    #[test]
    fn mojo_metadata_shape_keeps_provider_specific_error_precedence() {
        let request = json!({
            "metadata": {"deepseek": "bad"},
            "client_metadata": [],
            "prompt_cache_key": 42,
        });
        assert_eq!(
            deepseek_provider_core_response_metadata_from_responses_request(
                &request, "DeepSeek", "deepseek",
            )
            .unwrap_err(),
            "DeepSeek request metadata.deepseek must be an object"
        );

        let request = json!({
            "metadata": {},
            "client_metadata": [],
            "prompt_cache_key": 42,
        });
        assert_eq!(
            deepseek_provider_core_response_metadata_from_responses_request(
                &request, "DeepSeek", "deepseek",
            )
            .unwrap_err(),
            "DeepSeek client_metadata must be an object"
        );
    }

    #[test]
    fn mojo_json_guidance_handles_case_and_decoded_escape_content() {
        let mut messages = vec![json!({
            "role": "user",
            "content": "Return J\u{53}ON please"
        })];
        deepseek_provider_core_ensure_json_prompt_instruction(&mut messages);
        assert_eq!(messages.len(), 1);

        let mut messages = vec![json!({
            "role": "assistant",
            "content": "JSON"
        })];
        deepseek_provider_core_ensure_json_prompt_instruction(&mut messages);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "Respond with valid JSON only.");
    }

    #[test]
    fn thinking_note_preserves_non_object_metadata() {
        let mut metadata = Some(json!(["unrelated"]));
        deepseek_provider_core_note_thinking_tool_choice_omission(
            &json!({"tool_choice": "required"}),
            true,
            "DeepSeek",
            "deepseek",
            &mut metadata,
        );
        assert_eq!(metadata, Some(json!(["unrelated"])));
    }

    #[test]
    fn oversized_thinking_note_keeps_existing_metadata() {
        let original = Some(json!({"keep": true}));
        let mut metadata = original.clone();
        deepseek_provider_core_note_thinking_tool_choice_omission(
            &json!({"tool_choice": {"name": "x".repeat(4 * 1024 * 1024)}}),
            true,
            "DeepSeek",
            "deepseek",
            &mut metadata,
        );
        assert_eq!(metadata, original);
    }
}
