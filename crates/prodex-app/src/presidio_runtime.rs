use anyhow::{Context, Result};
use prodex_core::AppPaths;
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
    #[serde(default = "default_presidio_language")]
    language: String,
    #[serde(default = "default_presidio_fail_mode")]
    fail_mode: String,
}

impl Default for ProdexPresidioRuntimeFileConfig {
    fn default() -> Self {
        Self {
            analyzer_url: default_presidio_analyzer_url(),
            anonymizer_url: default_presidio_anonymizer_url(),
            language: default_presidio_language(),
            fail_mode: default_presidio_fail_mode(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePresidioRedactionConfig {
    pub(crate) analyzer_url: String,
    pub(crate) anonymizer_url: String,
    pub(crate) language: String,
    pub(crate) fail_closed: bool,
}

fn default_presidio_analyzer_url() -> String {
    DEFAULT_PRESIDIO_ANALYZER_URL.to_string()
}

fn default_presidio_anonymizer_url() -> String {
    DEFAULT_PRESIDIO_ANONYMIZER_URL.to_string()
}

fn default_presidio_language() -> String {
    DEFAULT_PRESIDIO_LANGUAGE.to_string()
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
    Ok(RuntimePresidioRedactionConfig {
        analyzer_url: file_config.analyzer_url,
        anonymizer_url: file_config.anonymizer_url,
        language: file_config.language,
        fail_closed: file_config.fail_mode.eq_ignore_ascii_case("closed"),
    })
}
