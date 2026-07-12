use clap::{Args, Subcommand, ValueEnum};
use std::fmt;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum PresidioCommands {
    /// Show configured Presidio endpoints and probe service health.
    Doctor(PresidioDoctorArgs),
    /// Show stored Presidio integration settings.
    Status,
    /// Analyze and anonymize text using Presidio.
    Redact(PresidioRedactArgs),
    /// Enable the local Presidio integration for future Prodex features.
    Enable(PresidioEnableArgs),
    /// Disable the local Presidio integration.
    Disable,
}

#[derive(Args)]
pub struct PresidioDoctorArgs {
    /// Override the Presidio Analyzer service URL.
    #[arg(long, value_name = "URL")]
    pub analyzer_url: Option<String>,
    /// Override the Presidio Anonymizer service URL.
    #[arg(long, value_name = "URL")]
    pub anonymizer_url: Option<String>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

impl fmt::Debug for PresidioDoctorArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PresidioDoctorArgs")
            .field("analyzer_url_configured", &self.analyzer_url.is_some())
            .field("anonymizer_url_configured", &self.anonymizer_url.is_some())
            .field("json", &self.json)
            .finish()
    }
}

#[derive(Args)]
pub struct PresidioRedactArgs {
    /// File to redact. If omitted, Prodex reads stdin unless --text is set.
    #[arg(value_name = "PATH", conflicts_with = "text")]
    pub path: Option<PathBuf>,
    /// Text to redact directly.
    #[arg(long, value_name = "TEXT", conflicts_with = "path")]
    pub text: Option<String>,
    /// Analyzer language code (for 'fixed' mode, or a fallback for 'auto').
    /// Deprecated: use --languages instead.
    #[arg(long, value_name = "LANG", conflicts_with = "languages")]
    pub language: Option<String>,
    /// Comma-separated list of analyzer language codes (e.g., "en,id").
    /// If omitted, defaults to the config file or "en".
    #[arg(long, value_name = "LANGS", value_delimiter = ',')]
    pub languages: Vec<String>,
    /// Language detection mode: fixed (default), auto, multi.
    #[arg(long, value_enum, value_name = "MODE", default_value_t = PresidioLanguageMode::Fixed)]
    pub language_mode: PresidioLanguageMode,
    /// Override the Presidio Analyzer service URL.
    #[arg(long, value_name = "URL")]
    pub analyzer_url: Option<String>,
    /// Override the Presidio Anonymizer service URL.
    #[arg(long, value_name = "URL")]
    pub anonymizer_url: Option<String>,
    /// Emit the full anonymizer JSON response.
    #[arg(long)]
    pub json: bool,
}

impl fmt::Debug for PresidioRedactArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PresidioRedactArgs")
            .field("path_configured", &self.path.is_some())
            .field("text_configured", &self.text.is_some())
            .field("language", &self.language)
            .field("languages", &self.languages)
            .field("language_mode", &self.language_mode)
            .field("analyzer_url_configured", &self.analyzer_url.is_some())
            .field("anonymizer_url_configured", &self.anonymizer_url.is_some())
            .field("json", &self.json)
            .finish()
    }
}

#[derive(Args)]
pub struct PresidioEnableArgs {
    /// Presidio Analyzer service URL.
    #[arg(long, default_value = "http://localhost:5002", value_name = "URL")]
    pub analyzer_url: String,
    /// Presidio Anonymizer service URL.
    #[arg(long, default_value = "http://localhost:5001", value_name = "URL")]
    pub anonymizer_url: String,
    /// Default Analyzer language code (for 'fixed' mode, or a fallback for 'auto').
    /// Deprecated: use --languages instead.
    #[arg(long, value_name = "LANG", conflicts_with = "languages")]
    pub language: Option<String>,
    /// Comma-separated list of analyzer language codes (e.g., "en,id").
    /// If omitted, defaults to "en".
    #[arg(long, value_name = "LANGS", value_delimiter = ',')]
    pub languages: Vec<String>,
    /// Language detection mode: fixed (default), auto, multi.
    #[arg(long, value_enum, value_name = "MODE", default_value_t = PresidioLanguageMode::Fixed)]
    pub language_mode: PresidioLanguageMode,
    /// Runtime failure behavior for future prompt-redaction hooks.
    #[arg(long, default_value = "open", value_enum)]
    pub fail_mode: PresidioFailMode,
}

impl fmt::Debug for PresidioEnableArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PresidioEnableArgs")
            .field("analyzer_url_configured", &!self.analyzer_url.is_empty())
            .field(
                "anonymizer_url_configured",
                &!self.anonymizer_url.is_empty(),
            )
            .field("language", &self.language)
            .field("languages", &self.languages)
            .field("language_mode", &self.language_mode)
            .field("fail_mode", &self.fail_mode)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PresidioFailMode {
    Open,
    Closed,
}

impl PresidioFailMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

#[derive(
    Clone, Copy, Debug, ValueEnum, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum PresidioLanguageMode {
    #[default]
    Fixed,
    Auto,
    Multi,
}

impl PresidioLanguageMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Auto => "auto",
            Self::Multi => "multi",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presidio_command_debug_redacts_endpoints_and_text() {
        let sentinel = "presidio-cli-debug-secret-sentinel";
        let commands = [
            PresidioCommands::Doctor(PresidioDoctorArgs {
                analyzer_url: Some(format!("https://user:{sentinel}@example.test")),
                anonymizer_url: None,
                json: false,
            }),
            PresidioCommands::Redact(PresidioRedactArgs {
                path: None,
                text: Some(sentinel.to_string()),
                language: None,
                languages: vec!["en".to_string()],
                language_mode: PresidioLanguageMode::Fixed,
                analyzer_url: None,
                anonymizer_url: Some(format!("https://example.test?token={sentinel}")),
                json: false,
            }),
            PresidioCommands::Enable(PresidioEnableArgs {
                analyzer_url: format!("https://user:{sentinel}@example.test"),
                anonymizer_url: "https://example.test".to_string(),
                language: None,
                languages: vec!["en".to_string()],
                language_mode: PresidioLanguageMode::Fixed,
                fail_mode: PresidioFailMode::Closed,
            }),
        ];

        for command in commands {
            let rendered = format!("{command:?}");
            assert!(rendered.contains("url_configured: true"), "{rendered}");
            assert!(!rendered.contains(sentinel), "{rendered}");
        }
    }
}
