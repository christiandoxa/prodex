//! DeepSeek request translation.

use std::collections::BTreeMap;

use super::deepseek_passthrough_endpoint;
use crate::mojo_json::Document;
use crate::translator::{ProviderTransformInput, ProviderTransformResult};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat};
use prodex_mojo_core::rich::{
    deepseek_request_issue_message, deepseek_responses_request_transform,
};
use serde_json::Value;

struct DeepSeekRequestPlan {
    body: Vec<u8>,
    degraded: bool,
    continuation: Option<Value>,
    omitted_tool_choice: Option<Value>,
}

fn deepseek_request_plan_from_responses(
    value: Value,
    model: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<DeepSeekRequestPlan, String> {
    let mut context = serde_json::Map::new();
    context.insert("request".to_string(), value);
    if let Some(model) = model {
        context.insert(
            "adapter_model".to_string(),
            Value::String(model.to_string()),
        );
    }
    if let Some(turn_state) = turn_state {
        context.insert(
            "turn_state".to_string(),
            Value::String(turn_state.to_string()),
        );
    }
    if let Some(session_id) = session_id {
        context.insert(
            "session_id".to_string(),
            Value::String(session_id.to_string()),
        );
    }
    let context = Value::Object(context);
    let mut document = Document::default();
    document.push(&context, None, "");
    let source = std::str::from_utf8(&document.raw)
        .map_err(|error| format!("DeepSeek request serialization failed: {error}"))?;
    let bytes = deepseek_responses_request_transform(&document.nodes, source)
        .map_err(|error| format!("DeepSeek request kernel failed: {error:?}"))?;
    let plan: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("DeepSeek request plan returned invalid JSON: {error}"))?;
    let issue = plan
        .get("issue")
        .and_then(Value::as_u64)
        .ok_or_else(|| "DeepSeek request plan omitted issue code".to_string())?;
    if issue != 0 {
        // The compiled transform owns issue selection and its compiled message
        // owner owns the diagnostic. Rust only checks unknown ABI tags.
        if !(1..=21).contains(&issue) {
            return Err(format!(
                "DeepSeek request plan returned unknown issue code {issue}"
            ));
        }
        let detail = plan
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(deepseek_request_issue_message(issue as i64, detail)
            .map_err(|error| format!("DeepSeek request issue rendering failed: {error:?}"))?);
    }
    let body = plan
        .get("body")
        .ok_or_else(|| "DeepSeek request plan omitted body".to_string())?;
    let body = serde_json::to_vec(body)
        .map_err(|error| format!("DeepSeek request plan body serialization failed: {error}"))?;
    let degraded = plan
        .get("degraded")
        .and_then(Value::as_bool)
        .ok_or_else(|| "DeepSeek request plan omitted degradation state".to_string())?;
    let continuation = plan
        .get("continuation")
        .filter(|value| !value.is_null())
        .cloned();
    let omitted_tool_choice = plan.get("omitted_tool_choice").cloned();
    Ok(DeepSeekRequestPlan {
        body,
        degraded,
        continuation,
        omitted_tool_choice,
    })
}

pub(super) fn deepseek_transform_request(
    provider: ProviderId,
    input: ProviderTransformInput,
) -> ProviderTransformResult {
    if deepseek_passthrough_endpoint(input.endpoint) {
        return ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            input.body,
        );
    }
    if !matches!(
        input.endpoint,
        ProviderEndpoint::Responses | ProviderEndpoint::ResponsesCompact
    ) {
        return ProviderTransformResult::unsupported(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
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
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                format!("failed to parse Responses request JSON: {error}"),
            );
        }
    };
    if !value.is_object() {
        return ProviderTransformResult::rejected(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            "DeepSeek request body must be a JSON object",
        );
    };
    let turn_state = input.headers.get("x-codex-turn-state").map(String::as_str);
    let session_id = input.headers.get("session_id").map(String::as_str);
    let DeepSeekRequestPlan {
        body,
        degraded,
        continuation,
        omitted_tool_choice,
    } = match deepseek_request_plan_from_responses(
        value,
        input.model.as_deref(),
        turn_state,
        session_id,
    ) {
        Ok(result) => result,
        Err(reason) => {
            return ProviderTransformResult::rejected(
                provider,
                input.endpoint,
                ProviderWireFormat::OpenAiResponses,
                ProviderWireFormat::OpenAiChatCompletions,
                reason,
            );
        }
    };
    let result = if degraded {
        ProviderTransformResult::degraded(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
            "DeepSeek degrades JSON schema output to json_object",
            BTreeMap::from([
                ("from".to_string(), Value::String("json_schema".to_string())),
                ("to".to_string(), Value::String("json_object".to_string())),
            ]),
        )
    } else {
        ProviderTransformResult::lossless(
            provider,
            input.endpoint,
            ProviderWireFormat::OpenAiResponses,
            ProviderWireFormat::OpenAiChatCompletions,
            body,
        )
    };
    let result = if let Some(tool_choice) = omitted_tool_choice {
        result.with_metadata(
            "deepseek",
            serde_json::json!({
                "omitted_tool_choice": {
                    "from": tool_choice,
                    "reason": "DeepSeek thinking mode currently rejects explicit tool_choice on the OpenAI Chat route, so Prodex omits it while preserving translated function tools"
                }
            }),
        )
    } else {
        result
    };
    if let Some(continuation) = continuation {
        result.with_metadata("continuation", continuation)
    } else {
        result
    }
}

#[cfg(test)]
#[path = "request_transform/tests.rs"]
mod tests;
