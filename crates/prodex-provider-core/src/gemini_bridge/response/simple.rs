//! Gemini simple-response fast-path classification.

use prodex_mojo_core::rich::{
    GeminiSimplePartAction, GeminiSimplePartInput, GeminiSimpleResponsePlan,
    plan_gemini_simple_response,
};

pub fn gemini_provider_core_simple_response(
    value: &serde_json::Value,
    mut blocked_tool_call_message: impl FnMut(&str, &serde_json::Value) -> Option<String>,
) -> bool {
    let parts = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let keys = parts
        .iter()
        .map(|part| {
            part.as_object()
                .map(|object| object.keys().map(String::as_str).collect::<Vec<_>>())
        })
        .collect::<Vec<_>>();
    let inputs = parts
        .iter()
        .zip(&keys)
        .map(|(part, keys)| {
            let function_call = part.get("functionCall");
            let function_object = function_call.and_then(serde_json::Value::as_object);
            GeminiSimplePartInput {
                keys: keys.as_deref(),
                text_present: part.get("text").is_some(),
                text_is_string: part
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .is_some(),
                thought: part
                    .get("thought")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                function_call_present: function_call.is_some(),
                function_call_is_object: function_object.is_some(),
                function_name: function_object
                    .and_then(|call| call.get("name"))
                    .and_then(serde_json::Value::as_str),
            }
        })
        .collect::<Vec<_>>();
    match plan_gemini_simple_response(&inputs).expect("Mojo Gemini simple-response plan failed") {
        GeminiSimpleResponsePlan::MessageOnly => true,
        GeminiSimpleResponsePlan::CheckParts(actions) => {
            for ((part, input), action) in parts.iter().zip(&inputs).zip(actions) {
                match action {
                    GeminiSimplePartAction::Reject => return false,
                    GeminiSimplePartAction::Skip => continue,
                    GeminiSimplePartAction::CheckCall
                    | GeminiSimplePartAction::CheckCallThenReject => {
                        let name = input
                            .function_name
                            .expect("Mojo callback plan requires a function name");
                        let args = part
                            .get("functionCall")
                            .and_then(|call| call.get("args"))
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({}));
                        if blocked_tool_call_message(name, &args).is_some()
                            || action == GeminiSimplePartAction::CheckCallThenReject
                        {
                            return false;
                        }
                    }
                }
            }
            true
        }
    }
}

#[cfg(test)]
#[path = "simple_tests.rs"]
mod tests;
