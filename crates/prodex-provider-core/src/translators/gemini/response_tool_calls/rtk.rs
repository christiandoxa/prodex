//! RTK shell-command wrapping for Gemini tool-call arguments.

use crate::translators::tool_args::wrap_json_string_arg_with;

pub(crate) fn gemini_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    if !matches!(name, "shell" | "exec_command") {
        return arguments.to_string();
    }
    wrap_json_string_arg_with(
        arguments,
        &["cmd", "command"],
        gemini_rtk_wrapped_shell_command,
    )
}

fn gemini_rtk_wrapped_shell_command(command: &str) -> Option<String> {
    prodex_mojo_core::gemini_tooling_policy::gemini_rtk_shell_command(command)
        .expect("compiled Mojo Gemini RTK shell wrapping failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn gemini_mojo_rtk_wrapping_is_reachable_from_json_tool_arguments() {
        let args = json!({"cmd":"pwd", "command":"cd /repo; cargo test", "other":42}).to_string();
        let actual: Value =
            serde_json::from_str(&gemini_rtk_wrapped_tool_arguments("exec_command", &args))
                .unwrap();
        assert_eq!(
            actual,
            json!({"cmd":"pwd", "command":"cd /repo; rtk cargo test", "other":42})
        );
        assert_eq!(gemini_rtk_wrapped_tool_arguments("read", &args), args);
        assert_eq!(
            gemini_rtk_wrapped_tool_arguments("shell", "invalid json"),
            "invalid json"
        );
        let unchanged = r#"{  "cmd": "pwd" }"#;
        assert_eq!(
            gemini_rtk_wrapped_tool_arguments("shell", unchanged),
            unchanged
        );
    }
}
