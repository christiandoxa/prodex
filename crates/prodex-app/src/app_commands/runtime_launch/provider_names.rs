pub(super) fn runtime_external_provider_has_rotation_summary(provider: &str) -> bool {
    matches!(
        provider.to_ascii_lowercase().as_str(),
        "gemini"
            | "gemini-oauth"
            | "anthropic"
            | "claude"
            | "copilot"
            | "github-copilot"
            | "github_copilot"
            | "deepseek"
    )
}
