pub use prodex_provider_core::ProviderModelSpec as GeminiModelSpec;

use prodex_provider_core::{ProviderId, provider_model_catalog, provider_model_fallback_chain};

pub const GEMINI_DEFAULT_MODEL: &str = prodex_provider_core::PRODEX_GEMINI_DEFAULT_MODEL;
pub const GEMINI_CHAT_COMPRESSION_MODEL: &str =
    prodex_provider_core::PRODEX_GEMINI_CHAT_COMPRESSION_MODEL;
pub const GEMINI_DEFAULT_CONTEXT_WINDOW: usize =
    prodex_provider_core::PRODEX_GEMINI_DEFAULT_CONTEXT_WINDOW;
pub const GEMINI_DEFAULT_AUTO_COMPACT_LIMIT: usize =
    prodex_provider_core::PRODEX_GEMINI_DEFAULT_AUTO_COMPACT_LIMIT;

pub fn gemini_model_catalog() -> &'static [GeminiModelSpec] {
    provider_model_catalog(ProviderId::Gemini)
}

pub fn gemini_model_spec(model: &str) -> Option<&'static GeminiModelSpec> {
    let model = model.trim();
    provider_model_catalog(ProviderId::Gemini)
        .iter()
        .find(|spec| spec.id == model)
}

pub fn gemini_model_fallback_chain(model: &str) -> Vec<String> {
    provider_model_fallback_chain(ProviderId::Gemini, model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_fallback_covers_current_flash_aliases() {
        assert_eq!(
            gemini_model_fallback_chain("flash"),
            vec![
                "gemini-3-flash-preview",
                "gemini-3.5-flash",
                "gemini-3-flash",
                "gemini-2.5-flash"
            ]
        );
        assert_eq!(
            gemini_model_fallback_chain("gemini-3.1-pro-preview-customtools"),
            vec![
                "gemini-3.1-pro-preview-customtools",
                "gemini-3.1-pro-preview",
                "gemini-3-pro-preview",
                "gemini-2.5-pro",
                "gemini-3-flash-preview",
                "gemini-3-flash",
                "gemini-3.5-flash",
                "gemini-2.5-flash",
            ]
        );
    }

    #[test]
    fn gemini_catalog_covers_current_codex_visible_models() {
        assert!(gemini_model_spec(GEMINI_DEFAULT_MODEL).is_some());
        assert!(gemini_model_spec("gemini-3.5-flash").is_some());
        assert!(gemini_model_spec("gemini-3.1-pro-preview-customtools").is_some());
    }
}
