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
    let detail = || detail(&source, plan).unwrap_or_default();
    match plan.tag {
        0 => Ok(()),
        1 => Err(format!("{provider_label} tools must be an array")),
        2 => Err(format!("{provider_label} tools entries must be objects")),
        3 => Err(format!(
            "{provider_label} tool type `{}` is not supported",
            detail()
        )),
        4 => Err(format!(
            "{provider_label} tool description must be a string"
        )),
        5 => Err(format!(
            "{provider_label} function description must be a string"
        )),
        6 => Err(format!(
            "{provider_label} function strict must be a boolean"
        )),
        7 => Err(format!("{provider_label} tool strict must be a boolean")),
        8 => Err(format!(
            "{provider_label} {} tools require a name",
            detail()
        )),
        9 => Err(format!(
            "{provider_label} custom tools cannot preserve strict=true through the function wrapper"
        )),
        10 => Err(format!(
            "{provider_label} custom tool format must be an object"
        )),
        11 => Err(format!(
            "{provider_label} custom tool format.type must be a string"
        )),
        12 => Err(format!("{provider_label} namespace tools require a name")),
        13 => Err(format!(
            "{provider_label} namespace tool `{}` requires a tools array",
            detail()
        )),
        14 => Err(format!(
            "{provider_label} namespace tool `{}` requires at least one tool",
            detail()
        )),
        15 => Err(format!(
            "{provider_label} namespace tool `{}` entries must be objects",
            detail()
        )),
        16 => Err(format!(
            "{provider_label} namespace function description must be a string"
        )),
        17 => Err(format!(
            "{provider_label} namespace function strict must be a boolean"
        )),
        18 => Err(format!(
            "{provider_label} namespace tool `{}` entries must be function tools",
            detail()
        )),
        19 => Err(format!(
            "{provider_label} namespace tool `{}` function entries require a name",
            detail()
        )),
        20 => Err(format!(
            "{provider_label} MCP function tools require a name"
        )),
        21 => Err(format!(
            "{provider_label} MCP function tool `{}` requires a schema",
            detail()
        )),
        22 => Err(format!(
            "{provider_label} MCP toolsets require a server name"
        )),
        23 => Err(format!(
            "{provider_label} MCP toolset `{}` requires allowed_tools or enabled configs",
            detail()
        )),
        _ => Err(format!(
            "{provider_label} tools shape validation returned an unknown result"
        )),
    }
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
