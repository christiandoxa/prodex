//! RTK shell-command argument wrapping.

#[path = "rtk/shell.rs"]
mod shell;

pub(crate) use self::shell::rtk_prefixed_noisy_shell_command;

pub(crate) fn chat_compatible_rtk_wrapped_tool_arguments(name: &str, arguments: &str) -> String {
    if !matches!(name, "shell" | "exec_command") {
        return arguments.to_string();
    }
    super::wrap_json_string_arg_with(arguments, &["cmd", "command"], |command| {
        prodex_mojo_core::rtk_noisy::wrapped_shell_command(command)
            .unwrap_or_else(|error| panic!("Mojo RTK rewrite failed: {error:?}"))
    })
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

    #[test]
    fn chat_compatible_rtk_wrapped_tool_arguments_respects_shell_syntax() {
        for (command, expected) in [
            (
                "printf 'a; b' && cargo test",
                "printf 'a; b' && rtk cargo test",
            ),
            ("pwd; cargo test", "pwd; rtk cargo test"),
            ("pwd||cargo test", "pwd||rtk cargo test"),
            ("pwd | cargo test", "pwd | rtk cargo test"),
            ("pwd\ncargo test", "pwd\nrtk cargo test"),
            (
                "RUST_LOG=é\u{3000}cargo\u{3000}test",
                "RUST_LOG=é\u{3000}rtk cargo\u{3000}test",
            ),
            ("echo 'cargo test'", "rtk echo 'cargo test'"),
            ("git 'cargo test'", "git 'cargo test'"),
            ("git status", "rtk git status"),
            ("git branch", "git branch"),
        ] {
            let arguments = chat_compatible_rtk_wrapped_tool_arguments(
                "shell",
                &serde_json::json!({"cmd": command}).to_string(),
            );
            let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();
            assert_eq!(arguments["cmd"], expected);
        }
    }
}
