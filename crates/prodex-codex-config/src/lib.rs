use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use std::env;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexModelProviderSource {
    ConfigFile,
    ProfileV2ConfigFile,
    CliOverride,
}

impl CodexModelProviderSource {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::ConfigFile => "config.toml",
            Self::ProfileV2ConfigFile => "profile-v2 config",
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
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
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

fn parse_toml_document_string_value(contents: &str, key: &str) -> Option<String> {
    let value = toml::from_str::<toml::Value>(contents).ok()?;
    let mut current = &value;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    match current {
        toml::Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn codex_config_file_value(config_path: &Path, key: &str) -> Option<String> {
    let contents = fs::read_to_string(config_path).ok()?;
    parse_toml_document_string_value(&contents, key)
        .or_else(|| parse_toml_string_assignment(&contents, key))
        .filter(|value| !value.trim().is_empty())
}

pub fn codex_config_value(codex_home: &Path, key: &str) -> Option<String> {
    codex_config_file_value(&codex_home.join("config.toml"), key)
}

fn codex_config_file_exact_value(config_path: &Path, key: &str) -> Option<String> {
    let contents = fs::read_to_string(config_path).ok()?;
    parse_toml_document_string_value(&contents, key)
        .or_else(|| parse_toml_string_assignment(&contents, key))
}

pub fn codex_config_exact_value(codex_home: &Path, key: &str) -> Option<String> {
    codex_config_file_exact_value(&codex_home.join("config.toml"), key)
}

pub fn is_valid_codex_profile_v2_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub fn codex_profile_v2_config_path(codex_home: &Path, profile_v2_name: &str) -> Option<PathBuf> {
    is_valid_codex_profile_v2_name(profile_v2_name)
        .then(|| codex_home.join(format!("{profile_v2_name}.config.toml")))
}

pub fn codex_cli_profile_v2_name(args: &[OsString]) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        let Some(arg) = args[index].to_str() else {
            index += 1;
            continue;
        };
        let profile_v2_name = if matches!(arg, "--profile" | "--profile-v2") {
            index += 1;
            args.get(index).and_then(|value| value.to_str())
        } else {
            arg.strip_prefix("--profile=")
                .or_else(|| arg.strip_prefix("--profile-v2="))
        };
        if let Some(profile_v2_name) =
            profile_v2_name.filter(|name| is_valid_codex_profile_v2_name(name))
        {
            return Some(profile_v2_name.to_string());
        }
        index += 1;
    }
    None
}

pub fn codex_config_value_with_profile_v2(
    codex_home: &Path,
    key: &str,
    profile_v2_name: Option<&str>,
) -> Option<String> {
    profile_v2_name
        .and_then(|profile_v2_name| codex_profile_v2_config_path(codex_home, profile_v2_name))
        .and_then(|config_path| codex_config_file_value(&config_path, key))
        .or_else(|| codex_config_value(codex_home, key))
}

pub fn codex_config_value_for_args(
    codex_home: &Path,
    args: &[OsString],
    key: &str,
) -> Option<String> {
    let profile_v2_name = codex_cli_profile_v2_name(args);
    codex_config_value_with_profile_v2(codex_home, key, profile_v2_name.as_deref())
}

pub fn codex_configured_model_provider(codex_home: &Path) -> Option<String> {
    codex_config_value(codex_home, "model_provider")
}

pub fn codex_configured_model_provider_with_profile_v2(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> Option<String> {
    codex_config_value_with_profile_v2(codex_home, "model_provider", profile_v2_name)
}

pub fn codex_configured_model_provider_for_args(
    codex_home: &Path,
    args: &[OsString],
) -> Option<String> {
    codex_config_value_for_args(codex_home, args, "model_provider")
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

pub fn codex_cli_config_override_exact_value(args: &[OsString], key: &str) -> Option<String> {
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
        if let Some(value) =
            assignment.and_then(|value| parse_config_override_exact_string(value, key))
        {
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
    codex_non_openai_model_provider_with_profile_v2(codex_home, model_provider_override, None)
}

pub fn codex_non_openai_model_provider_with_profile_v2(
    codex_home: &Path,
    model_provider_override: Option<&str>,
    profile_v2_name: Option<&str>,
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
        .or_else(|| codex_model_provider_setting_from_config(codex_home, profile_v2_name))?;
    (!provider.is_openai()).then_some(provider)
}

fn codex_model_provider_setting_from_config(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> Option<CodexModelProviderSetting> {
    if let Some(provider_id) = profile_v2_name
        .and_then(|profile_v2_name| codex_profile_v2_config_path(codex_home, profile_v2_name))
        .and_then(|config_path| codex_config_file_value(&config_path, "model_provider"))
    {
        return Some(CodexModelProviderSetting {
            provider_id,
            source: CodexModelProviderSource::ProfileV2ConfigFile,
        });
    }

    codex_configured_model_provider(codex_home).map(|provider_id| CodexModelProviderSetting {
        provider_id,
        source: CodexModelProviderSource::ConfigFile,
    })
}

pub fn codex_non_openai_model_provider_for_args(
    codex_home: &Path,
    args: &[OsString],
) -> Option<CodexModelProviderSetting> {
    let model_provider_override = codex_cli_config_override_value(args, "model_provider");
    let profile_v2_name = codex_cli_profile_v2_name(args);
    codex_non_openai_model_provider_with_profile_v2(
        codex_home,
        model_provider_override.as_deref(),
        profile_v2_name.as_deref(),
    )
}

fn parse_config_override_string(assignment: &str, expected_key: &str) -> Option<String> {
    let (key, raw_value) = assignment.split_once('=')?;
    if key.trim() != expected_key {
        return None;
    }
    normalize_model_provider_value(raw_value)
}

fn parse_config_override_exact_string(assignment: &str, expected_key: &str) -> Option<String> {
    let (key, raw_value) = assignment.split_once('=')?;
    if key.trim() != expected_key {
        return None;
    }
    let raw_value = raw_value.trim();
    Some(
        parse_toml_string_assignment(&format!("{expected_key} = {raw_value}"), expected_key)
            .unwrap_or_else(|| raw_value.to_string()),
    )
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
