use prodex_mojo_core::super_provider_config::{
    RuntimeExternalProviderClass,
    runtime_external_provider_class as mojo_runtime_external_provider_class,
};

pub(super) fn runtime_external_provider_class(
    provider: &str,
) -> Option<RuntimeExternalProviderClass> {
    mojo_runtime_external_provider_class(provider)
        .expect("runtime external provider classification should accept Rust strings")
}

pub(super) fn runtime_external_provider_has_rotation_summary(provider: &str) -> bool {
    matches!(
        runtime_external_provider_class(provider),
        Some(
            RuntimeExternalProviderClass::Gemini
                | RuntimeExternalProviderClass::GeminiOauth
                | RuntimeExternalProviderClass::Anthropic
                | RuntimeExternalProviderClass::Copilot
                | RuntimeExternalProviderClass::DeepSeek
                | RuntimeExternalProviderClass::Kiro
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_external_provider_has_rotation_summary_uses_mojo_alias_policy() {
        for provider in [
            "kiro",
            "CLAUDE",
            "github_copilot",
            "deepseek",
            "gemini-oauth",
        ] {
            assert!(runtime_external_provider_has_rotation_summary(provider));
        }
        for provider in ["unknown", " gemini ", "gemini-native", "antigravity"] {
            assert!(!runtime_external_provider_has_rotation_summary(provider));
        }
    }
}
