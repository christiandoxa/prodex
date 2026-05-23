use clap::{Args, Subcommand, ValueEnum};
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

#[derive(Args, Debug)]
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

#[derive(Args, Debug)]
pub struct PresidioRedactArgs {
    /// File to redact. If omitted, Prodex reads stdin unless --text is set.
    #[arg(value_name = "PATH", conflicts_with = "text")]
    pub path: Option<PathBuf>,
    /// Text to redact directly.
    #[arg(long, value_name = "TEXT", conflicts_with = "path")]
    pub text: Option<String>,
    /// Analyzer language code.
    #[arg(long, default_value = "en", value_name = "LANG")]
    pub language: String,
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

#[derive(Args, Debug)]
pub struct PresidioEnableArgs {
    /// Presidio Analyzer service URL.
    #[arg(long, default_value = "http://localhost:5002", value_name = "URL")]
    pub analyzer_url: String,
    /// Presidio Anonymizer service URL.
    #[arg(long, default_value = "http://localhost:5001", value_name = "URL")]
    pub anonymizer_url: String,
    /// Default Analyzer language code.
    #[arg(long, default_value = "en", value_name = "LANG")]
    pub language: String,
    /// Runtime failure behavior for future prompt-redaction hooks.
    #[arg(long, default_value = "open", value_enum)]
    pub fail_mode: PresidioFailMode,
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
