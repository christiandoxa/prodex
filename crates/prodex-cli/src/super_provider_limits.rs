pub use prodex_provider_core::{
    PRODEX_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT as SUPER_COPILOT_DEFAULT_AUTO_COMPACT_LIMIT,
    PRODEX_COPILOT_DEFAULT_CONTEXT_WINDOW as SUPER_COPILOT_DEFAULT_CONTEXT_WINDOW,
    copilot_prompt_token_limit_for_model as super_copilot_prompt_token_limit_for_model,
};

pub(crate) fn external_provider_token_limits(
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
    let context_window = context_window
        .filter(|value| *value > 1)
        .unwrap_or(default_context_window);
    let default_auto_compact_token_limit = if is_copilot {
        context_window.saturating_mul(95).saturating_div(100)
    } else {
        default_auto_compact_token_limit
    };
    let auto_compact_token_limit = auto_compact_token_limit
        .filter(|value| *value > 0)
        .unwrap_or(default_auto_compact_token_limit)
        .min(context_window.saturating_sub(1));
    (context_window, auto_compact_token_limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_prompt_limit_for_default_codex_model() {
        assert_eq!(
            super_copilot_prompt_token_limit_for_model("gpt-5.3-codex"),
            Some(272_000)
        );
    }

    #[test]
    fn copilot_auto_compact_tracks_effective_context() {
        assert_eq!(
            external_provider_token_limits("gpt-5.4", true, 272_000, 258_400, None, None),
            (922_000, 875_900)
        );
    }
}
