//! Gemini model fallback aliases and Code Assist filtering.

pub(super) fn provider_gemini_model_fallback_alias_chain(
    lower: &str,
) -> Option<&'static [&'static str]> {
    Some(match lower {
        "chat-compression-default" => &[
            "gemini-3-pro-preview",
            "gemini-3-flash-preview",
            "gemini-2.5-pro",
            "gemini-2.5-flash",
        ],
        "" | "auto" | "auto-gemini-3" => &[
            "gemini-3-pro-preview",
            "gemini-3.1-pro-preview",
            "gemini-2.5-pro",
            "gemini-3-flash-preview",
            "gemini-3.5-flash",
            "gemini-3-flash",
            "gemini-2.5-flash",
        ],
        "auto-gemini-2.5" => &["gemini-2.5-pro", "gemini-2.5-flash"],
        "pro" => &[
            "gemini-3-pro-preview",
            "gemini-3.1-pro-preview",
            "gemini-2.5-pro",
        ],
        "gemini-3.1-pro-preview-customtools" => &[
            "gemini-3.1-pro-preview-customtools",
            "gemini-3.1-pro-preview",
            "gemini-3-pro-preview",
            "gemini-2.5-pro",
            "gemini-3-flash-preview",
            "gemini-3-flash",
            "gemini-3.5-flash",
            "gemini-2.5-flash",
        ],
        "gemini-3.1-pro-preview" => &[
            "gemini-3.1-pro-preview",
            "gemini-3-pro-preview",
            "gemini-2.5-pro",
            "gemini-3-flash-preview",
            "gemini-3-flash",
            "gemini-3.5-flash",
            "gemini-2.5-flash",
        ],
        "gemini-3-pro-preview" => &[
            "gemini-3-pro-preview",
            "gemini-3.1-pro-preview",
            "gemini-2.5-pro",
            "gemini-3-flash-preview",
            "gemini-3.5-flash",
            "gemini-3-flash",
            "gemini-2.5-flash",
        ],
        "gemini-3.5-flash" => &[
            "gemini-3.5-flash",
            "gemini-3-flash",
            "gemini-3-flash-preview",
            "gemini-2.5-flash",
        ],
        "gemini-3-flash-preview" => &[
            "gemini-3-flash-preview",
            "gemini-3.5-flash",
            "gemini-3-flash",
            "gemini-2.5-flash",
        ],
        "gemini-3-flash" => &["gemini-3-flash", "gemini-3.5-flash", "gemini-2.5-flash"],
        "gemini-3.1-flash-lite" => &[
            "gemini-3.1-flash-lite",
            "gemini-2.5-flash-lite",
            "gemini-2.5-flash",
        ],
        "flash" => &[
            "gemini-3-flash-preview",
            "gemini-3.5-flash",
            "gemini-3-flash",
            "gemini-2.5-flash",
        ],
        "flash-lite" => &["gemini-3.1-flash-lite", "gemini-2.5-flash-lite"],
        _ => return None,
    })
}

pub(super) fn provider_gemini_code_assist_model_allowed(model: &str) -> bool {
    let model = model.trim();
    !model.contains("customtools") && !matches!(model, "gemini-3.5-flash" | "gemini-3-flash")
}
