use super::super::{RuntimeConfigParser, RuntimeGeminiConfig};
use prodex_mojo_core::runtime::{
    RuntimeConfigScalarStatus, runtime_config_scalar_compat_bool_default_true,
};

pub(super) fn parse_gemini(parser: &mut RuntimeConfigParser) -> RuntimeGeminiConfig {
    let key = "PRODEX_GEMINI_STICKY_FRESH_OAUTH";
    let (sticky_fresh_oauth, status) =
        runtime_config_scalar_compat_bool_default_true(parser.scalar_input(key))
            .expect("required Mojo Gemini compatibility scalar returned an invalid ABI result");
    if status == RuntimeConfigScalarStatus::CompatibilityDefault {
        parser.compatibility_defaults.push(key);
    }
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
