use anyhow::{Result, bail};
use prodex_mojo_core::super_provider_config::{CatalogUnsignedPlan, catalog_unsigned_value};

pub(crate) fn parse_catalog_u64(provider: &str, key: &str, value: &str) -> Result<u64> {
    match catalog_unsigned_value(value)
        .map_err(|error| anyhow::anyhow!("{provider} {key} validation failed: {error:?}"))?
    {
        CatalogUnsignedPlan::Value(value) => Ok(value),
        CatalogUnsignedPlan::Empty => bail!("{provider} {key} cannot be empty"),
        CatalogUnsignedPlan::Whitespace => bail!("{provider} {key} must not contain whitespace"),
        CatalogUnsignedPlan::InvalidUnsignedInteger { reason } => Err(anyhow::anyhow!(reason)
            .context(format!("{provider} {key} must be an unsigned integer"))),
        CatalogUnsignedPlan::TooSmall => bail!("{provider} {key} must be greater than 1"),
    }
}

pub(crate) fn toml_string_literal(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_catalog_u64, toml_string_literal};

    #[test]
    fn catalog_unsigned_values_preserve_provider_messages_and_integer_causes() {
        for provider in ["DeepSeek", "Gemini", "local provider"] {
            for key in ["model_context_window", "model_auto_compact_token_limit"] {
                for (input, message) in [
                    ("", "cannot be empty"),
                    ("2\u{a0}", "must not contain whitespace"),
                    ("1", "must be greater than 1"),
                    (
                        "-0",
                        "must be an unsigned integer: invalid digit found in string",
                    ),
                    (
                        "18446744073709551616x",
                        "must be an unsigned integer: number too large to fit in target type",
                    ),
                    (
                        "1844674407370955161x6",
                        "must be an unsigned integer: invalid digit found in string",
                    ),
                ] {
                    let error = parse_catalog_u64(provider, key, input).unwrap_err();
                    assert_eq!(format!("{error:#}"), format!("{provider} {key} {message}"));
                }
                assert_eq!(parse_catalog_u64(provider, key, "+0002").unwrap(), 2);
                assert_eq!(
                    parse_catalog_u64(provider, key, "18446744073709551615").unwrap(),
                    u64::MAX
                );
            }
        }
    }

    #[test]
    fn toml_string_literal_escapes_control_characters() {
        let literal = toml_string_literal("catalog\n\"name\"");
        assert_eq!(
            toml::from_str::<toml::Value>(&format!("value = {literal}"))
                .unwrap()
                .get("value")
                .and_then(toml::Value::as_str),
            Some("catalog\n\"name\"")
        );
    }
}
