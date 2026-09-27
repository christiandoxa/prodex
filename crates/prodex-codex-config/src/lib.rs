use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Read as _};
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

#[derive(Debug)]
pub enum CodexConfigError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

impl fmt::Display for CodexConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for CodexConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

pub type CodexConfigResult<T> = Result<T, CodexConfigError>;
const CODEX_CONFIG_MAX_BYTES: u64 = 1024 * 1024;

fn codex_mojo_args(args: &[OsString]) -> Vec<Option<&str>> {
    args.iter().map(|arg| arg.to_str()).collect()
}

impl CodexModelProviderSetting {
    pub fn is_openai(&self) -> bool {
        self.provider_id.eq_ignore_ascii_case("openai")
    }
}

fn parse_toml_document_value(
    contents: &str,
    key: &str,
) -> Result<Option<toml::Value>, toml::de::Error> {
    let value = toml::from_str::<toml::Value>(contents)?;
    let mut current = &value;
    for part in key.split('.') {
        let Some(value) = current.get(part) else {
            return Ok(None);
        };
        current = value;
    }
    Ok(Some(current.clone()))
}

fn parse_toml_document_string_value(
    contents: &str,
    key: &str,
) -> Result<Option<String>, toml::de::Error> {
    Ok(
        parse_toml_document_value(contents, key)?.and_then(|value| match value {
            toml::Value::String(value) => Some(value),
            _ => None,
        }),
    )
}

/// Reads a bounded TOML config file and returns the value at a dotted key.
pub fn codex_config_file_toml_value(
    config_path: &Path,
    key: &str,
) -> CodexConfigResult<Option<toml::Value>> {
    let Some(contents) = read_codex_config_file(config_path)? else {
        return Ok(None);
    };
    parse_toml_document_value(&contents, key).map_err(|source| CodexConfigError::Parse {
        path: config_path.to_path_buf(),
        source,
    })
}

fn codex_config_file_exact_value(
    config_path: &Path,
    key: &str,
) -> CodexConfigResult<Option<String>> {
    Ok(
        codex_config_file_toml_value(config_path, key)?.and_then(|value| match value {
            toml::Value::String(value) => Some(value),
            _ => None,
        }),
    )
}

fn read_codex_config_file(config_path: &Path) -> CodexConfigResult<Option<String>> {
    let file = match fs::File::open(config_path) {
        Ok(file) => file,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(CodexConfigError::Read {
                path: config_path.to_path_buf(),
                source,
            });
        }
    };
    let mut contents = String::new();
    file.take(CODEX_CONFIG_MAX_BYTES.saturating_add(1))
        .read_to_string(&mut contents)
        .map_err(|source| CodexConfigError::Read {
            path: config_path.to_path_buf(),
            source,
        })?;
    if contents.len() as u64 > CODEX_CONFIG_MAX_BYTES {
        return Err(CodexConfigError::Read {
            path: config_path.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::InvalidData,
                format!("config exceeds safe size limit ({CODEX_CONFIG_MAX_BYTES} bytes)"),
            ),
        });
    }
    Ok(Some(contents))
}

fn codex_config_file_value(config_path: &Path, key: &str) -> CodexConfigResult<Option<String>> {
    Ok(codex_config_file_exact_value(config_path, key)?.filter(|value| !value.trim().is_empty()))
}

pub fn codex_config_value(codex_home: &Path, key: &str) -> CodexConfigResult<Option<String>> {
    codex_config_file_value(&codex_home.join("config.toml"), key)
}

pub fn codex_config_exact_value(codex_home: &Path, key: &str) -> CodexConfigResult<Option<String>> {
    codex_config_file_exact_value(&codex_home.join("config.toml"), key)
}

pub fn is_valid_codex_profile_v2_name(name: &str) -> bool {
    prodex_mojo_core::codex_config::profile_name_valid(name)
        .expect("Mojo Codex profile-v2 name validation returned invalid output")
}

pub fn codex_profile_v2_config_path(codex_home: &Path, profile_v2_name: &str) -> Option<PathBuf> {
    is_valid_codex_profile_v2_name(profile_v2_name)
        .then(|| codex_home.join(format!("{profile_v2_name}.config.toml")))
}

pub fn codex_cli_profile_v2_name(args: &[OsString]) -> Option<String> {
    prodex_mojo_core::codex_config::profile_v2_name(&codex_mojo_args(args))
        .expect("Mojo Codex profile-v2 argument scan returned invalid output")
}

pub fn codex_config_value_with_profile_v2(
    codex_home: &Path,
    key: &str,
    profile_v2_name: Option<&str>,
) -> CodexConfigResult<Option<String>> {
    if let Some(value) = profile_v2_name
        .and_then(|profile_v2_name| codex_profile_v2_config_path(codex_home, profile_v2_name))
        .map(|config_path| codex_config_file_value(&config_path, key))
        .transpose()?
        .flatten()
    {
        return Ok(Some(value));
    }
    codex_config_value(codex_home, key)
}

pub fn codex_config_value_for_args(
    codex_home: &Path,
    args: &[OsString],
    key: &str,
) -> CodexConfigResult<Option<String>> {
    let profile_v2_name = codex_cli_profile_v2_name(args);
    codex_config_value_with_profile_v2(codex_home, key, profile_v2_name.as_deref())
}

fn codex_config_exact_value_for_args(
    codex_home: &Path,
    args: &[OsString],
    key: &str,
) -> CodexConfigResult<Option<String>> {
    if let Some(value) = codex_cli_profile_v2_name(args)
        .and_then(|name| codex_profile_v2_config_path(codex_home, &name))
        .map(|path| codex_config_file_exact_value(&path, key))
        .transpose()?
        .flatten()
    {
        return Ok(Some(value));
    }
    codex_config_exact_value(codex_home, key)
}

pub fn codex_configured_model_provider(codex_home: &Path) -> CodexConfigResult<Option<String>> {
    codex_config_value(codex_home, "model_provider")
}

#[cfg(test)]
fn codex_configured_model_provider_with_profile_v2(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> CodexConfigResult<Option<String>> {
    codex_config_value_with_profile_v2(codex_home, "model_provider", profile_v2_name)
}

pub fn codex_cli_config_override_value(args: &[OsString], key: &str) -> Option<String> {
    prodex_mojo_core::codex_config::config_override(&codex_mojo_args(args), key)
        .expect("Mojo Codex config override scan returned invalid output")
        .and_then(|selection| selection.normalized_value)
}

pub fn codex_cli_config_override_exact_value(args: &[OsString], key: &str) -> Option<String> {
    let selection = prodex_mojo_core::codex_config::config_override(&codex_mojo_args(args), key)
        .expect("Mojo Codex exact config override scan returned invalid output")?;
    let raw_value = selection.raw_value.trim();
    parse_toml_document_string_value(&format!("{key} = {raw_value}"), key)
        .ok()
        .flatten()
}

pub fn codex_effective_config_value(
    codex_home: &Path,
    args: &[OsString],
    key: &str,
) -> CodexConfigResult<Option<String>> {
    match codex_cli_config_override_value(args, key) {
        Some(value) => Ok(Some(value)),
        None => codex_config_value_for_args(codex_home, args, key),
    }
}

pub fn codex_effective_config_exact_value(
    codex_home: &Path,
    args: &[OsString],
    key: &str,
) -> CodexConfigResult<Option<String>> {
    match codex_cli_config_override_exact_value(args, key) {
        Some(value) => Ok(Some(value)),
        None => codex_config_exact_value_for_args(codex_home, args, key),
    }
}

pub fn codex_non_openai_model_provider(
    codex_home: &Path,
    model_provider_override: Option<&str>,
) -> CodexConfigResult<Option<CodexModelProviderSetting>> {
    codex_non_openai_model_provider_with_profile_v2(codex_home, model_provider_override, None)
}

pub fn codex_non_openai_model_provider_with_profile_v2(
    codex_home: &Path,
    model_provider_override: Option<&str>,
    profile_v2_name: Option<&str>,
) -> CodexConfigResult<Option<CodexModelProviderSetting>> {
    let Some(provider) = model_provider_override
        .and_then(|provider_id| {
            normalize_model_provider_value(provider_id).map(|provider_id| {
                CodexModelProviderSetting {
                    provider_id,
                    source: CodexModelProviderSource::CliOverride,
                }
            })
        })
        .map(Some)
        .unwrap_or(codex_model_provider_setting_from_config(
            codex_home,
            profile_v2_name,
        )?)
    else {
        return Ok(None);
    };
    Ok((!provider.is_openai()).then_some(provider))
}

fn codex_model_provider_setting_from_config(
    codex_home: &Path,
    profile_v2_name: Option<&str>,
) -> CodexConfigResult<Option<CodexModelProviderSetting>> {
    if let Some(provider_id) = profile_v2_name
        .and_then(|profile_v2_name| codex_profile_v2_config_path(codex_home, profile_v2_name))
        .map(|config_path| codex_config_file_value(&config_path, "model_provider"))
        .transpose()?
        .flatten()
    {
        return Ok(Some(CodexModelProviderSetting {
            provider_id,
            source: CodexModelProviderSource::ProfileV2ConfigFile,
        }));
    }

    Ok(
        codex_configured_model_provider(codex_home)?.map(|provider_id| CodexModelProviderSetting {
            provider_id,
            source: CodexModelProviderSource::ConfigFile,
        }),
    )
}

#[cfg(test)]
fn codex_non_openai_model_provider_for_args(
    codex_home: &Path,
    args: &[OsString],
) -> CodexConfigResult<Option<CodexModelProviderSetting>> {
    let model_provider_override = codex_cli_config_override_value(args, "model_provider");
    let profile_v2_name = codex_cli_profile_v2_name(args);
    codex_non_openai_model_provider_with_profile_v2(
        codex_home,
        model_provider_override.as_deref(),
        profile_v2_name.as_deref(),
    )
}

fn normalize_model_provider_value(raw_value: &str) -> Option<String> {
    prodex_mojo_core::codex_config::normalize_value(raw_value)
        .expect("Mojo Codex model-provider normalization returned invalid output")
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
