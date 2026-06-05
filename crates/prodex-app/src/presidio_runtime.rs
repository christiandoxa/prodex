use anyhow::{Context, Result};
pub use prodex_cli::PresidioLanguageMode;
use prodex_core::AppPaths;
use serde::{Deserialize, Deserializer};
use std::fs;

const PRODEX_PRESIDIO_FILE_NAME: &str = "presidio.toml";
const DEFAULT_PRESIDIO_ANALYZER_URL: &str = "http://localhost:5002";
const DEFAULT_PRESIDIO_ANONYMIZER_URL: &str = "http://localhost:5001";
const DEFAULT_PRESIDIO_LANGUAGE: &str = "en";

#[derive(Debug, Clone, serde::Deserialize)]
struct ProdexPresidioRuntimeFileConfig {
    #[serde(default = "default_presidio_analyzer_url")]
    analyzer_url: String,
    #[serde(default = "default_presidio_anonymizer_url")]
    anonymizer_url: String,
    language: Option<String>,
    languages: Option<Vec<String>>,
    #[serde(
        default = "default_presidio_language_mode_str",
        deserialize_with = "deserialize_presidio_language_mode"
    )]
    language_mode: PresidioLanguageMode,
    #[serde(default = "default_presidio_fail_mode")]
    fail_mode: String,
}

impl Default for ProdexPresidioRuntimeFileConfig {
    fn default() -> Self {
        Self {
            analyzer_url: default_presidio_analyzer_url(),
            anonymizer_url: default_presidio_anonymizer_url(),
            language: None,
            languages: None,
            language_mode: PresidioLanguageMode::default(),
            fail_mode: default_presidio_fail_mode(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePresidioRedactionConfig {
    pub(crate) analyzer_url: String,
    pub(crate) anonymizer_url: String,
    pub(crate) languages: Vec<String>,
    pub(crate) language_mode: PresidioLanguageMode,
    pub(crate) fail_closed: bool,
}

fn default_presidio_language_mode_str() -> PresidioLanguageMode {
    PresidioLanguageMode::Fixed
}

fn deserialize_presidio_language_mode<'de, D>(
    deserializer: D,
) -> Result<PresidioLanguageMode, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "fixed" => Ok(PresidioLanguageMode::Fixed),
        "auto" => Ok(PresidioLanguageMode::Auto),
        "multi" => Ok(PresidioLanguageMode::Multi),
        _ => Err(serde::de::Error::custom(format!(
            "unknown Presidio language mode: {}",
            s
        ))),
    }
}

fn default_presidio_analyzer_url() -> String {
    DEFAULT_PRESIDIO_ANALYZER_URL.to_string()
}

fn default_presidio_anonymizer_url() -> String {
    DEFAULT_PRESIDIO_ANONYMIZER_URL.to_string()
}

fn default_presidio_fail_mode() -> String {
    "open".to_string()
}

pub(crate) fn runtime_presidio_redaction_config(
    paths: &AppPaths,
) -> Result<RuntimePresidioRedactionConfig> {
    let path = paths.root.join(PRODEX_PRESIDIO_FILE_NAME);
    let file_config = if path.exists() {
        toml::from_str::<ProdexPresidioRuntimeFileConfig>(
            &fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?,
        )
        .with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        ProdexPresidioRuntimeFileConfig::default()
    };

    let languages = file_config.languages.unwrap_or_else(|| {
        file_config
            .language
            .map(|l| vec![l])
            .unwrap_or_else(|| vec![DEFAULT_PRESIDIO_LANGUAGE.to_string()])
    });

    let language_mode = file_config.language_mode;

    if language_mode == PresidioLanguageMode::Fixed && languages.len() != 1 {
        anyhow::bail!(
            "Fixed Presidio language mode requires exactly one language, found: {:?}",
            languages
        );
    }

    Ok(RuntimePresidioRedactionConfig {
        analyzer_url: file_config.analyzer_url,
        anonymizer_url: file_config.anonymizer_url,
        languages,
        language_mode,
        fail_closed: file_config.fail_mode.eq_ignore_ascii_case("closed"),
    })
}
