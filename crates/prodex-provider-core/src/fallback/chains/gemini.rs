//! Gemini Code Assist model filtering.

pub(super) fn provider_gemini_code_assist_model_allowed(model: &str) -> bool {
    let model = model.trim();
    !model.contains("customtools") && !matches!(model, "gemini-3.5-flash" | "gemini-3-flash")
}
