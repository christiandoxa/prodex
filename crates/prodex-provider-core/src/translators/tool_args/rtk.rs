//! RTK shell-command argument wrapping.

use serde_json::Value;

#[path = "rtk/shell.rs"]
mod shell;

pub(crate) use self::shell::rtk_prefixed_noisy_shell_command;
use self::shell::rtk_wrapped_shell_command;

pub(crate) fn chat_compatible_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
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
            .and_then(rtk_wrapped_shell_command)
        else {
            continue;
        };
        object.insert(key.to_string(), Value::String(wrapped));
        return serde_json::to_string(&value).unwrap_or_else(|_| arguments.to_string());
    }
    arguments.to_string()
}

#[cfg(test)]
mod tests {
    use super::chat_compatible_rtk_wrapped_tool_arguments;

    #[test]
    fn chat_compatible_rtk_wrapped_tool_arguments_wraps_noisy_segment() {
        let arguments = chat_compatible_rtk_wrapped_tool_arguments(
            "shell",
            r#"{"cmd":"cd /repo && cargo check -q"}"#,
        );
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(arguments["cmd"], "cd /repo && rtk cargo check -q");
    }

    #[test]
    fn chat_compatible_rtk_wrapped_tool_arguments_keeps_quiet_command() {
        let arguments = chat_compatible_rtk_wrapped_tool_arguments("shell", r#"{"cmd":"pwd"}"#);
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(arguments["cmd"], "pwd");
    }
}
