use anyhow::{Context, Result, bail};

pub(crate) fn parse_catalog_u64(provider: &str, key: &str, value: &str) -> Result<u64> {
    if value.is_empty() {
        bail!("{provider} {key} cannot be empty");
    }
    if value.chars().any(char::is_whitespace) {
        bail!("{provider} {key} must not contain whitespace");
    }
    let parsed = value
        .parse::<u64>()
        .with_context(|| format!("{provider} {key} must be an unsigned integer"))?;
    if parsed <= 1 {
        bail!("{provider} {key} must be greater than 1");
    }
    Ok(parsed)
}

pub(crate) fn toml_string_literal(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
