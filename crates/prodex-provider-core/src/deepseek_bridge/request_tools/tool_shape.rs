//! DeepSeek top-level tools-array shape validation.

use crate::deepseek_bridge::request_policy::{detail, try_plan_value};
use prodex_mojo_core::rich::DeepSeekRequestPolicyOperation;

pub fn deepseek_provider_core_validate_tools_shape(
    value: &serde_json::Value,
    gemini_compat: bool,
    provider_label: &str,
) -> Result<(), String> {
    if !value.is_object() {
        return Ok(());
    }
    let (source, plan) = try_plan_value(
        value,
        DeepSeekRequestPolicyOperation::ToolsShape,
        gemini_compat,
        provider_label,
    )?;
    if plan.tag == 0 {
        return Ok(());
    }
    let reason = prodex_mojo_core::rich::deepseek_tool_shape_issue_message(
        plan.tag,
        provider_label,
        &detail(&source, plan).unwrap_or_default(),
    )
    .map_err(|error| format!("DeepSeek tools shape message ABI failed: {error:?}"))?;
    Err(reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_preserves_first_error_and_unicode_blank_mcp_checks() {
        deepseek_provider_core_validate_tools_shape(
            &serde_json::json!({"tools": [{"type": 7}]}),
            false,
            "DeepSeek",
        )
        .unwrap();

        let error = deepseek_provider_core_validate_tools_shape(
            &serde_json::json!({
                "tools": [
                    {"type": "file_search"},
                    {"type": "custom", "name": "patch", "strict": true}
                ]
            }),
            false,
            "DeepSeek",
        )
        .unwrap_err();
        assert_eq!(error, "DeepSeek tool type `file_search` is not supported");

        let error = deepseek_provider_core_validate_tools_shape(
            &serde_json::json!({
                "tools": [{
                    "type": "mcp_toolset",
                    "server_name": "server",
                    "allowed_tools": ["\u{2003}"]
                }]
            }),
            false,
            "DeepSeek",
        )
        .unwrap_err();
        assert_eq!(
            error,
            "DeepSeek MCP toolset `server` requires allowed_tools or enabled configs"
        );
    }

    #[test]
    fn mojo_tool_shape_diagnostics_preserve_deepseek_and_kiro_provider_labels() {
        for provider in ["DeepSeek", "Kiro"] {
            for (request, expected_suffix) in [
                (
                    serde_json::json!({"tools": "bad"}),
                    " tools must be an array",
                ),
                (
                    serde_json::json!({"tools": [{"type": "file_search"}]}),
                    " tool type `file_search` is not supported",
                ),
                (
                    serde_json::json!({"tools": [{
                        "type": "mcp_toolset",
                        "server_name": "東京🧭",
                        "allowed_tools": ["\u{2003}"],
                    }]}),
                    " MCP toolset `東京🧭` requires allowed_tools or enabled configs",
                ),
            ] {
                assert_eq!(
                    deepseek_provider_core_validate_tools_shape(&request, false, provider),
                    Err(format!("{provider}{expected_suffix}")),
                    "provider={provider:?} request={request}",
                );
            }
        }
    }

    #[test]
    fn validation_accepts_limit_and_rejects_one_byte_over() {
        let mut value = serde_json::json!({"tools": [], "padding": ""});
        let size = serde_json::to_string(&value).unwrap().len();
        let padding = prodex_mojo_core::rich::DEEPSEEK_KERNEL_MAX_BYTES - size;
        value["padding"] = serde_json::Value::String("x".repeat(padding));
        assert_eq!(
            serde_json::to_string(&value).unwrap().len(),
            prodex_mojo_core::rich::DEEPSEEK_KERNEL_MAX_BYTES
        );
        deepseek_provider_core_validate_tools_shape(&value, false, "DeepSeek").unwrap();

        let mut padding = value["padding"].as_str().unwrap().to_owned();
        padding.push('x');
        value["padding"] = serde_json::Value::String(padding);
        assert_eq!(
            deepseek_provider_core_validate_tools_shape(&value, false, "DeepSeek").unwrap_err(),
            format!(
                "DeepSeek request policy input exceeds {} bytes",
                prodex_mojo_core::rich::DEEPSEEK_KERNEL_MAX_BYTES
            )
        );
    }
}
