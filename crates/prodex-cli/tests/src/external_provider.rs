use super::*;

#[test]
fn super_external_providers_enable_live_web_search() {
    for provider in ["anthropic", "copilot", "deepseek", "gemini"] {
        let args = parse_super_as_caveman(&[
            "prodex",
            "s",
            "--provider",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "review",
        ]);
        let rendered = args
            .codex_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(
            rendered.contains(&"web_search=\"live\"".to_string()),
            "{provider} should enable live web search"
        );
    }
}
