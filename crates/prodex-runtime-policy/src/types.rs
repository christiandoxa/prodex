use serde::{Deserialize, Deserializer, de};
use std::path::PathBuf;

mod runtime_proxy_preset;
pub use runtime_proxy_preset::RuntimePolicyProxySettings;

pub const PRODEX_POLICY_FILE_NAME: &str = "policy.toml";
pub const PRODEX_POLICY_VERSION: u32 = 1;
pub const PRODEX_RUNTIME_PROXY_PRESET_ENV: &str = "PRODEX_RUNTIME_PROXY_PRESET";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeLogFormat {
    Text,
    Json,
}

impl RuntimeLogFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimePolicySummary {
    pub path: PathBuf,
    pub version: u32,
}

#[derive(Debug, Clone)]
pub struct RuntimePolicyConfig {
    pub path: PathBuf,
    pub version: u32,
    pub runtime: RuntimePolicyRuntimeSettings,
    pub runtime_proxy: RuntimePolicyProxySettings,
    pub secrets: RuntimePolicySecretsSettings,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimePolicyRuntimeSettings {
    pub log_format: Option<RuntimeLogFormat>,
    pub log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimePolicySecretsSettings {
    pub backend: Option<secret_store::SecretBackendKind>,
    pub keyring_service: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePolicyProxyPreset {
    Low,
    Default,
    ManyTerminals,
    Aggressive,
}

impl RuntimePolicyProxyPreset {
    pub const VALID_VALUES: &'static [&'static str] =
        &["low", "default", "many-terminals", "aggressive"];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Default => "default",
            Self::ManyTerminals => "many-terminals",
            Self::Aggressive => "aggressive",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value != value.trim() {
            return None;
        }
        match value.to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "default" => Some(Self::Default),
            "many-terminals" | "many_terminals" => Some(Self::ManyTerminals),
            "aggressive" => Some(Self::Aggressive),
            _ => None,
        }
    }

    pub(super) fn settings(self) -> RuntimePolicyProxySettings {
        self.resolve()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimePolicyProxyPresetSelection(Option<RuntimePolicyProxyPreset>);

impl RuntimePolicyProxyPresetSelection {
    pub fn selected(preset: RuntimePolicyProxyPreset) -> Self {
        Self(Some(preset))
    }

    pub fn get(self) -> Option<RuntimePolicyProxyPreset> {
        self.0
    }
}

impl<'de> Deserialize<'de> for RuntimePolicyProxyPresetSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        RuntimePolicyProxyPreset::parse(&value)
            .map(Self::selected)
            .ok_or_else(|| {
                de::Error::unknown_variant(value.as_str(), RuntimePolicyProxyPreset::VALID_VALUES)
            })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimePolicyFile {
    pub version: u32,
    #[serde(default)]
    pub runtime: RuntimePolicyRuntimeFile,
    #[serde(default)]
    pub runtime_proxy: RuntimePolicyProxySettings,
    #[serde(default)]
    pub secrets: RuntimePolicySecretsFile,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RuntimePolicyRuntimeFile {
    pub log_format: Option<RuntimeLogFormat>,
    pub log_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RuntimePolicySecretsFile {
    pub backend: Option<String>,
    pub keyring_service: Option<String>,
}
