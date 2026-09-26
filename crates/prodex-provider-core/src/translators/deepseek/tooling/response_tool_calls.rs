//! DeepSeek response tool-call normalization helpers.

use crate::translators::tool_args::{rtk_prefixed_noisy_shell_command, wrap_json_string_arg_with};
use prodex_mojo_core::rich::{
    DEEPSEEK_KERNEL_MAX_BYTES, DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES, DeepSeekKernelInput,
    DeepSeekKernelOperation, deepseek_kernel,
};
use serde_json::Value;

pub(crate) fn deepseek_responses_tool_call_item(
    tool_call: &Value,
) -> Result<Option<Value>, String> {
    let Some(function) = tool_call.get("function").and_then(Value::as_object) else {
        return Err("DeepSeek returned a tool call without a function object".to_string());
    };
    let Some(name) = function.get("name").and_then(Value::as_str) else {
        return Err("DeepSeek returned a tool call without a function name".to_string());
    };
    if name.len() > DEEPSEEK_KERNEL_MAX_BYTES {
        return Err("DeepSeek returned a tool call name exceeding the supported size".to_string());
    }
    if name.trim().is_empty() {
        return Err("DeepSeek returned a tool call without a function name".to_string());
    }
    let arguments = function
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    if arguments.len() > DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES {
        return Err("DeepSeek returned JSON arguments exceeding the supported size".to_string());
    }
    deepseek_validate_tool_call_arguments(name, arguments)?;
    let arguments = deepseek_rtk_wrapped_tool_arguments(name, arguments);
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::ResponseToolCallItem);
    input.call_id = tool_call.get("id").and_then(Value::as_str);
    input.name = Some(name);
    input.arguments = Some(&arguments);
    input.signature = tool_call
        .get("extra_content")
        .and_then(|value| value.get("google"))
        .and_then(|value| value.get("thought_signature"))
        .and_then(Value::as_str);
    let body = deepseek_kernel(input)
        .map_err(|_| "DeepSeek response tool call exceeds the bounded Mojo kernel limit")?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| format!("DeepSeek Mojo returned invalid response tool-call JSON: {error}"))
}

fn deepseek_validate_tool_call_arguments(name: &str, arguments: &str) -> Result<(), String> {
    if arguments.trim().is_empty() && name != "tool_search" {
        return Ok(());
    }
    serde_json::from_str::<Value>(arguments)
        .map(|_| ())
        .map_err(|error| deepseek_tool_call_arguments_error(name, error))
}

fn deepseek_tool_call_arguments_error(name: &str, error: serde_json::Error) -> String {
    format!("DeepSeek returned malformed JSON arguments for tool call `{name}`: {error}")
}

pub(crate) fn deepseek_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    if !matches!(name, "shell" | "exec" | "functions.exec_command") || arguments.trim().is_empty() {
        return arguments.to_string();
    }
    wrap_json_string_arg_with(arguments, &["cmd"], rtk_prefixed_noisy_shell_command)
}

#[cfg(test)]
mod tests {
    use super::{DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES, deepseek_responses_tool_call_item};
    use serde_json::json;

    #[test]
    fn response_tool_call_items_keep_provider_specific_shapes() {
        assert_eq!(
            deepseek_responses_tool_call_item(&json!({
                "id": "call_search",
                "function": {
                    "name": "tool_search",
                    "arguments": "{ \"query\": \"café\" }"
                }
            }))
            .unwrap(),
            Some(json!({
                "type": "tool_search_call",
                "call_id": "call_search",
                "execution": "client",
                "arguments": {"query": "café"}
            }))
        );
        assert_eq!(
            deepseek_responses_tool_call_item(&json!({
                "function": {
                    "name": "apply_patch",
                    "arguments": "{\"patch\":\"*** Begin Patch\\n*** End Patch\"}"
                }
            }))
            .unwrap(),
            Some(json!({
                "type": "custom_tool_call",
                "call_id": "call_0",
                "name": "apply_patch",
                "input": "{\"patch\":\"*** Begin Patch\\n*** End Patch\"}"
            }))
        );
        assert_eq!(
            deepseek_responses_tool_call_item(&json!({
                "id": "call_lookup",
                "function": {
                    "name": "functions__lookup",
                    "arguments": "{\"query\":\"東京\"}"
                },
                "extra_content": {"google": {"thought_signature": "sig-1"}}
            }))
            .unwrap(),
            Some(json!({
                "type": "function_call",
                "call_id": "call_lookup",
                "name": "lookup",
                "namespace": "functions",
                "arguments": "{\"query\":\"東京\"}",
                "gemini_thought_signature": "sig-1"
            }))
        );
    }

    #[test]
    fn response_tool_call_item_rejects_malformed_arguments() {
        assert_eq!(
            deepseek_responses_tool_call_item(&json!({
                "function": {"name": "lookup", "arguments": "{bad"}
            })),
            Err("DeepSeek returned malformed JSON arguments for tool call `lookup`: key must be a string at line 1 column 2".to_string())
        );
        assert_eq!(
            deepseek_responses_tool_call_item(&json!({
                "function": {"name": "tool_search", "arguments": ""}
            })),
            Err("DeepSeek returned malformed JSON arguments for tool call `tool_search`: EOF while parsing a value at line 1 column 0".to_string())
        );
        assert_eq!(
            deepseek_responses_tool_call_item(&json!({})),
            Err("DeepSeek returned a tool call without a function object".to_string())
        );
        assert_eq!(
            deepseek_responses_tool_call_item(&json!({"function": {}})),
            Err("DeepSeek returned a tool call without a function name".to_string())
        );
    }

    #[test]
    fn response_tool_call_item_splits_supported_namespace_separators() {
        for (source, namespace, name) in [
            ("functions__lookup", "functions", "lookup"),
            ("functions.lookup", "functions", "lookup"),
            ("functions/lookup", "functions", "lookup"),
            ("api.v1/lookup", "api", "v1/lookup"),
        ] {
            assert_eq!(
                deepseek_responses_tool_call_item(&json!({
                    "function": {"name": source}
                }))
                .unwrap(),
                Some(json!({
                    "type": "function_call",
                    "call_id": "call_0",
                    "name": name,
                    "namespace": namespace,
                    "arguments": "{}"
                }))
            );
        }
    }

    #[test]
    fn response_tool_call_item_accepts_arguments_across_four_mib_boundary() {
        let boundary = 4 * 1024 * 1024;
        for length in [boundary - 1, boundary + 1] {
            let arguments = format!("{{\"v\":\"{}\"}}", "x".repeat(length - 8));
            assert_eq!(arguments.len(), length);
            let tool_call = json!({
                "id": "call_large",
                "function": {"name": "lookup", "arguments": arguments}
            });
            assert!(tool_call.to_string().len() > boundary);
            assert_eq!(
                deepseek_responses_tool_call_item(&tool_call).unwrap(),
                Some(json!({
                    "type": "function_call",
                    "call_id": "call_large",
                    "name": "lookup",
                    "arguments": arguments
                }))
            );
        }
    }

    #[test]
    fn response_tool_call_item_ignores_large_unrelated_fields() {
        let tool_call = json!({
            "function": {"name": "lookup", "arguments": "{}"},
            "unrelated": "x".repeat(4 * 1024 * 1024 + 1)
        });
        assert!(tool_call.to_string().len() > 4 * 1024 * 1024);
        assert_eq!(
            deepseek_responses_tool_call_item(&tool_call).unwrap(),
            Some(json!({
                "type": "function_call",
                "call_id": "call_0",
                "name": "lookup",
                "arguments": "{}"
            }))
        );
    }

    #[test]
    fn response_tool_call_item_rejects_arguments_over_the_bounded_limit() {
        let arguments = format!(
            "{{\"v\":\"{}\"}}",
            "x".repeat(DEEPSEEK_LARGE_RESPONSE_KERNEL_MAX_BYTES)
        );
        let tool_call = json!({
            "function": {"name": "lookup", "arguments": arguments}
        });
        assert_eq!(
            deepseek_responses_tool_call_item(&tool_call),
            Err("DeepSeek returned JSON arguments exceeding the supported size".to_string())
        );
    }
}
