//! Gemini Code Assist model filter adapter.

pub(super) fn provider_gemini_code_assist_model_allowed(model: &str) -> bool {
    prodex_mojo_core::rich::gemini_code_assist_model_allowed(model)
        .expect("Mojo Gemini Code Assist model filter returned an invalid result")
}
