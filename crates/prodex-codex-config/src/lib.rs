use std::ffi::OsString;
use std::fs;
use std::path::Path;

#[cfg(test)]
use std::env;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexModelProviderSource {
    ConfigFile,
    CliOverride,
}

impl CodexModelProviderSource {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::ConfigFile => "config.toml",
            Self::CliOverride => "CLI override",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexModelProviderSetting {
    pub provider_id: String,
    pub source: CodexModelProviderSource,
}

impl CodexModelProviderSetting {
    pub fn is_openai(&self) -> bool {
        self.provider_id.eq_ignore_ascii_case("openai")
    }
}

pub fn parse_toml_string_assignment(contents: &str, key: &str) -> Option<String> {
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

pub fn codex_config_value(codex_home: &Path, key: &str) -> Option<String> {
    let contents = fs::read_to_string(codex_home.join("config.toml")).ok()?;
    parse_toml_string_assignment(&contents, key).filter(|value| !value.trim().is_empty())
}

pub fn codex_configured_model_provider(codex_home: &Path) -> Option<String> {
    codex_config_value(codex_home, "model_provider")
}

pub fn codex_cli_config_override_value(args: &[OsString], key: &str) -> Option<String> {
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

pub fn codex_non_openai_model_provider(
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
#[path = "../tests/src/lib.rs"]
mod tests;
