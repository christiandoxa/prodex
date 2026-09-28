use super::super::{RuntimeConfigParser, RuntimeGeminiConfig};
use prodex_mojo_core::super_provider_config::runtime_bool_token;

pub(super) fn parse_gemini(parser: &mut RuntimeConfigParser) -> RuntimeGeminiConfig {
    let sticky_fresh_oauth = parser
        .compatibility_text("PRODEX_GEMINI_STICKY_FRESH_OAUTH")
        .is_none_or(|value| {
            runtime_bool_token(value.trim())
                .expect("runtime boolean token classifier should accept Rust strings")
                != Some(false)
        });
    RuntimeGeminiConfig { sticky_fresh_oauth }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_config::RuntimeConfigEnvironment;
    use std::ffi::OsString;

    fn sticky_value(value: Option<&str>) -> bool {
        let environment = RuntimeConfigEnvironment::read_with(|key| {
            (key == "PRODEX_GEMINI_STICKY_FRESH_OAUTH")
                .then(|| value.map(OsString::from))
                .flatten()
        });
        let mut parser = RuntimeConfigParser::new(environment);
        parse_gemini(&mut parser).sticky_fresh_oauth
    }

    #[test]
    fn sticky_fresh_oauth_reuses_runtime_boolean_classifier() {
        assert!(sticky_value(None));
        for value in ["0", "FALSE", " off ", "No"] {
            assert!(!sticky_value(Some(value)), "{value}");
        }
        for value in ["1", "TRUE", " yes ", "on", "", "maybe"] {
            assert!(sticky_value(Some(value)), "{value}");
        }
    }
}
