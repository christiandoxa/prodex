#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeminiModelSpec {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub owned_by: &'static str,
}

pub const GEMINI_DEFAULT_MODEL: &str = "auto";
pub const GEMINI_DEFAULT_CONTEXT_WINDOW: usize = 1_048_576;
pub const GEMINI_DEFAULT_AUTO_COMPACT_LIMIT: usize = 900_000;

const GEMINI_MODEL_CATALOG: &[GeminiModelSpec] = &[
    GeminiModelSpec {
        id: "auto",
        display_name: "Gemini Auto",
        description: "Gemini CLI-style auto routing through preview, pro, and flash fallback models.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "auto-gemini-3",
        display_name: "Gemini 3 Auto",
        description: "Gemini CLI preview auto alias routed through Gemini 3 and stable fallback models.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "auto-gemini-2.5",
        display_name: "Gemini 2.5 Auto",
        description: "Gemini CLI stable auto alias routed through Gemini 2.5 Pro and Flash models.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "pro",
        display_name: "Gemini Pro",
        description: "Gemini Pro alias routed through preview and stable Pro models.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "flash",
        display_name: "Gemini Flash",
        description: "Gemini Flash alias routed through preview and stable Flash models.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "flash-lite",
        display_name: "Gemini Flash Lite",
        description: "Gemini Flash-Lite alias routed through Flash-Lite models.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-3.1-pro-preview",
        display_name: "Gemini 3.1 Pro Preview",
        description: "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-3.1-pro-preview-customtools",
        display_name: "Gemini 3.1 Pro Preview Custom Tools",
        description: "Gemini CLI preview Pro variant for custom tools routed through Prodex.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-3-pro-preview",
        display_name: "Gemini 3 Pro Preview",
        description: "Gemini CLI preview Pro model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-3-flash-preview",
        display_name: "Gemini 3 Flash Preview",
        description: "Gemini CLI preview Flash model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-3-flash",
        display_name: "Gemini 3 Flash",
        description: "Gemini CLI secondary Flash model name routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-3.5-flash",
        display_name: "Gemini 3.5 Flash",
        description: "Gemini CLI Flash model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-3.1-flash-lite",
        display_name: "Gemini 3.1 Flash Lite",
        description: "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-2.5-pro",
        display_name: "Gemini 2.5 Pro",
        description: "Gemini CLI stable Pro model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-2.5-flash",
        display_name: "Gemini 2.5 Flash",
        description: "Gemini CLI stable Flash model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemini-2.5-flash-lite",
        display_name: "Gemini 2.5 Flash Lite",
        description: "Gemini CLI Flash-Lite model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemma-4-31b-it",
        display_name: "Gemma 4 31B IT",
        description: "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
    GeminiModelSpec {
        id: "gemma-4-26b-a4b-it",
        display_name: "Gemma 4 26B A4B IT",
        description: "Gemini CLI Gemma model routed through the Prodex Responses adapter.",
        owned_by: "google",
    },
];

pub fn gemini_model_catalog() -> &'static [GeminiModelSpec] {
    GEMINI_MODEL_CATALOG
}

pub fn gemini_model_spec(model: &str) -> Option<&'static GeminiModelSpec> {
    let model = model.trim();
    GEMINI_MODEL_CATALOG.iter().find(|spec| spec.id == model)
}

pub fn gemini_model_fallback_chain(model: &str) -> Vec<String> {
    let model = model.trim();
    let lower = model.to_ascii_lowercase();
    let chain: &[&str] = match lower.as_str() {
        "" | "auto" | "auto-gemini-3" => &[
            "gemini-3.1-pro-preview",
            "gemini-3-pro-preview",
            "gemini-2.5-pro",
            "gemini-3-flash-preview",
            "gemini-3-flash",
            "gemini-3.5-flash",
            "gemini-2.5-flash",
        ],
        "auto-gemini-2.5" => &["gemini-2.5-pro", "gemini-2.5-flash"],
        "pro" => &[
            "gemini-3.1-pro-preview",
            "gemini-3-pro-preview",
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
            "gemini-3-flash",
            "gemini-3.5-flash",
            "gemini-2.5-flash",
        ],
        "gemini-3.5-flash" => &[
            "gemini-3.5-flash",
            "gemini-3-flash-preview",
            "gemini-3-flash",
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
            "gemini-3.5-flash",
            "gemini-3-flash-preview",
            "gemini-3-flash",
            "gemini-2.5-flash",
        ],
        "flash-lite" => &["gemini-3.1-flash-lite", "gemini-2.5-flash-lite"],
        _ => return vec![model.to_string()],
    };
    chain.iter().map(|model| (*model).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_fallback_covers_current_flash_aliases() {
        assert_eq!(
            gemini_model_fallback_chain("flash"),
            vec![
                "gemini-3.5-flash",
                "gemini-3-flash-preview",
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
