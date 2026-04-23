use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodexModelProviderSource {
    ConfigFile,
    CliOverride,
}

impl CodexModelProviderSource {
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::ConfigFile => "config.toml",
            Self::CliOverride => "CLI override",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexModelProviderSetting {
    pub(crate) provider_id: String,
    pub(crate) source: CodexModelProviderSource,
}

impl CodexModelProviderSetting {
    pub(crate) fn is_openai(&self) -> bool {
        self.provider_id.eq_ignore_ascii_case("openai")
    }
}

pub(crate) fn parse_toml_string_assignment(contents: &str, key: &str) -> Option<String> {
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('=')?.trim_start();
        let quote = match rest.chars().next()? {
            '"' | '\'' => rest.chars().next()?,
            _ => continue,
        };
        let mut value = String::new();
        let mut escaped = false;
        for ch in rest[quote.len_utf8()..].chars() {
            if quote == '"' && escaped {
                value.push(match ch {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                });
                escaped = false;
                continue;
            }
            match ch {
                '\\' if quote == '"' => escaped = true,
                ch if ch == quote => return Some(value),
                other => value.push(other),
            }
        }
    }
    None
}

pub(crate) fn codex_config_value(codex_home: &Path, key: &str) -> Option<String> {
    let contents = fs::read_to_string(codex_home.join("config.toml")).ok()?;
    parse_toml_string_assignment(&contents, key).filter(|value| !value.trim().is_empty())
}

pub(crate) fn codex_configured_model_provider(codex_home: &Path) -> Option<String> {
    codex_config_value(codex_home, "model_provider")
}

pub(crate) fn codex_cli_config_override_value(args: &[OsString], key: &str) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        let Some(arg) = args[index].to_str() else {
            index += 1;
            continue;
        };
        let assignment = if matches!(arg, "-c" | "--config") {
            index += 1;
            args.get(index)?.to_str()
        } else if let Some(value) = arg.strip_prefix("--config=") {
            Some(value)
        } else if let Some(value) = arg.strip_prefix("-c") {
            (!value.is_empty() && value.contains('=')).then_some(value)
        } else {
            None
        };
        if let Some(value) = assignment.and_then(|value| parse_config_override_string(value, key)) {
            return Some(value);
        }
        index += 1;
    }
    None
}

pub(crate) fn codex_non_openai_model_provider(
    codex_home: &Path,
    model_provider_override: Option<&str>,
) -> Option<CodexModelProviderSetting> {
    let provider = model_provider_override
        .and_then(|provider_id| {
            normalize_model_provider_value(provider_id).map(|provider_id| {
                CodexModelProviderSetting {
                    provider_id,
                    source: CodexModelProviderSource::CliOverride,
                }
            })
        })
        .or_else(|| {
            codex_configured_model_provider(codex_home).map(|provider_id| {
                CodexModelProviderSetting {
                    provider_id,
                    source: CodexModelProviderSource::ConfigFile,
                }
            })
        })?;
    (!provider.is_openai()).then_some(provider)
}

fn parse_config_override_string(assignment: &str, expected_key: &str) -> Option<String> {
    let (key, raw_value) = assignment.split_once('=')?;
    if key.trim() != expected_key {
        return None;
    }
    normalize_model_provider_value(raw_value)
}

fn normalize_model_provider_value(raw_value: &str) -> Option<String> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let unquoted = if trimmed.len() >= 2 {
        let first = trimmed.chars().next()?;
        let last = trimmed.chars().last()?;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    let normalized = unquoted.trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_provider_from_config_toml() {
        let contents = r#"
            model_provider = "amazon-bedrock"
            model = "gpt-5.4"
        "#;

        assert_eq!(
            parse_toml_string_assignment(contents, "model_provider").as_deref(),
            Some("amazon-bedrock")
        );
    }

    #[test]
    fn cli_override_takes_precedence_over_config_file() {
        let root = temp_dir("cli-override-precedence");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.toml"), "model_provider = 'openai'\n").unwrap();

        let provider = codex_non_openai_model_provider(
            &root,
            codex_cli_config_override_value(
                &[
                    OsString::from("--config"),
                    OsString::from("model_provider='amazon-bedrock'"),
                ],
                "model_provider",
            )
            .as_deref(),
        )
        .unwrap();

        assert_eq!(provider.provider_id, "amazon-bedrock");
        assert_eq!(provider.source, CodexModelProviderSource::CliOverride);
    }

    #[test]
    fn explicit_openai_override_clears_non_openai_config() {
        let root = temp_dir("explicit-openai-override");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("config.toml"),
            "model_provider = 'amazon-bedrock'\n",
        )
        .unwrap();

        let provider = codex_non_openai_model_provider(
            &root,
            codex_cli_config_override_value(
                &[OsString::from("--config=model_provider=openai")],
                "model_provider",
            )
            .as_deref(),
        );

        assert!(provider.is_none());
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "prodex-codex-config-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        dir
    }
}
