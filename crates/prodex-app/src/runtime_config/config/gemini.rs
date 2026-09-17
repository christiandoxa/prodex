use super::super::{RuntimeConfigParser, RuntimeGeminiConfig};

pub(super) fn parse_gemini(parser: &mut RuntimeConfigParser) -> RuntimeGeminiConfig {
    let sticky_fresh_oauth = parser
        .compatibility_text("PRODEX_GEMINI_STICKY_FRESH_OAUTH")
        .is_none_or(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            )
        });
    RuntimeGeminiConfig { sticky_fresh_oauth }
}
