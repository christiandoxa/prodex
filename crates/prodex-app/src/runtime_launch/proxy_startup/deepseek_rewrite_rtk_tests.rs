#[cfg(test)]
mod tests {
    use prodex_provider_core::deepseek_provider_core_rtk_wrapped_tool_arguments;

    #[test]
    fn deepseek_wraps_noisy_shell_arguments_with_rtk() {
        let arguments = deepseek_provider_core_rtk_wrapped_tool_arguments(
            "exec",
            r#"{"cmd":"cargo test -q login -- --test-threads=1"}"#,
        );
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(
            arguments["cmd"],
            "rtk cargo test -q login -- --test-threads=1"
        );
    }

    #[test]
    fn deepseek_wraps_noisy_shell_segment_after_cd() {
        let arguments = deepseek_provider_core_rtk_wrapped_tool_arguments(
            "exec",
            r#"{"cmd":"cd /home/test-user/workspace/prodex && cargo check -q"}"#,
        );
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(
            arguments["cmd"],
            "rtk cd /home/test-user/workspace/prodex && cargo check -q"
        );
    }

    #[test]
    fn deepseek_wraps_listing_shell_arguments_with_rtk() {
        for command in ["find . -maxdepth 2 -type f", "ls -la", "tree -a -L 3"] {
            let arguments = deepseek_provider_core_rtk_wrapped_tool_arguments(
                "exec",
                &serde_json::json!({ "cmd": command }).to_string(),
            );
            let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

            assert_eq!(arguments["cmd"], format!("rtk {command}"));
        }
    }

    #[test]
    fn deepseek_does_not_double_wrap_rtk_shell_arguments() {
        let arguments = deepseek_provider_core_rtk_wrapped_tool_arguments(
            "exec",
            r#"{"cmd":"rtk cargo test -q login"}"#,
        );
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(arguments["cmd"], "rtk cargo test -q login");
    }

    #[test]
    fn deepseek_keeps_quiet_shell_arguments_unchanged() {
        let arguments =
            deepseek_provider_core_rtk_wrapped_tool_arguments("exec", r#"{"cmd":"pwd"}"#);
        let arguments: serde_json::Value = serde_json::from_str(&arguments).unwrap();

        assert_eq!(arguments["cmd"], "pwd");
    }
}
