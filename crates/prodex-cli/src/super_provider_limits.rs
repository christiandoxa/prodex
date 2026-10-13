pub use prodex_provider_core::{
    PRODEX_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT as SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT,
    PRODEX_COPILOT_DEFAULT_CONTEXT_WINDOW as SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
    copilot_prompt_token_limit_for_model as super_copilot_prompt_token_limit_for_model,
};

pub(crate) fn provider_token_limits(
    model: &str,
    is_copilot: bool,
    default_context_window: usize,
    default_auto_compact_token_limit: usize,
    context_window: Option<usize>,
    auto_compact_token_limit: Option<usize>,
) -> (usize, usize) {
    let default_context_window = if is_copilot {
        super_copilot_prompt_token_limit_for_model(model).unwrap_or(default_context_window)
    } else {
        default_context_window
    };
    let normalized = prodex_mojo_core::super_provider_config::external_provider_token_limits(
        is_copilot,
        u64::try_from(default_context_window).expect("usize fits Mojo token-limit ABI"),
        u64::try_from(default_auto_compact_token_limit).expect("usize fits Mojo token-limit ABI"),
        context_window.map(|value| u64::try_from(value).expect("usize fits Mojo token-limit ABI")),
        auto_compact_token_limit
            .map(|value| u64::try_from(value).expect("usize fits Mojo token-limit ABI")),
    )
    .expect("Mojo Super provider token-limit policy returned invalid output");
    (
        usize::try_from(normalized.context_window)
            .expect("Mojo context window fits the host usize"),
        usize::try_from(normalized.auto_compact_token_limit)
            .expect("Mojo compact limit fits the host usize"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_prompt_limit_for_default_model() {
        assert_eq!(
            super_copilot_prompt_token_limit_for_model("gpt-6-astra"),
            Some(1_050_000)
        );
    }

    #[test]
    fn copilot_auto_compact_tracks_effective_context() {
        assert_eq!(
            provider_token_limits("gpt-5.4", true, 272_000, 258_400, None, None),
            (922_000, 875_900)
        );
    }
}
