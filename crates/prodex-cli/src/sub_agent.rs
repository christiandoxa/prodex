use prodex_provider_core::ProviderId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::num::NonZeroU16;
use std::str::FromStr;

pub const SUPER_SUB_AGENT_DEFAULT_PROVIDER: ProviderId = ProviderId::OpenAi;
pub const DEFAULT_SUB_AGENT_MAX_CONCURRENCY: u16 = 4;
pub const HARD_MAX_SUB_AGENT_CONCURRENCY: u16 = 64;
pub const SUB_AGENT_MAX_CONCURRENCY_PRESETS: [u16; 4] = [4, 8, 16, 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubAgentConcurrencySource {
    Default,
    Preset,
    Custom,
}

impl SubAgentConcurrencySource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "Prodex default",
            Self::Preset => "explicit preset",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    try_from = "SubAgentMaxConcurrencyWire",
    into = "SubAgentMaxConcurrencyWire"
)]
pub struct SubAgentMaxConcurrency {
    value: NonZeroU16,
    source: SubAgentConcurrencySource,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubAgentMaxConcurrencyWire {
    value: u16,
    source: SubAgentConcurrencySource,
}

impl SubAgentMaxConcurrency {
    pub fn new(value: u16, source: SubAgentConcurrencySource) -> Result<Self, String> {
        if value == 0 || value > HARD_MAX_SUB_AGENT_CONCURRENCY {
            return Err(format!(
                "maximum active sub-agents must be between 1 and {HARD_MAX_SUB_AGENT_CONCURRENCY}"
            ));
        }
        Ok(Self {
            value: NonZeroU16::new(value).expect("validated nonzero value"),
            source,
        })
    }

    pub const fn get(self) -> u16 {
        self.value.get()
    }

    pub const fn source(self) -> SubAgentConcurrencySource {
        self.source
    }
}

impl Default for SubAgentMaxConcurrency {
    fn default() -> Self {
        Self::new(
            DEFAULT_SUB_AGENT_MAX_CONCURRENCY,
            SubAgentConcurrencySource::Default,
        )
        .expect("built-in sub-agent concurrency default must be valid")
    }
}

impl FromStr for SubAgentMaxConcurrency {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value == "default" {
            return Ok(Self::default());
        }
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!(
                "invalid maximum active sub-agents: expected default or an integer from 1 to {HARD_MAX_SUB_AGENT_CONCURRENCY}"
            ));
        }
        let parsed = value.parse::<u16>().map_err(|_| {
            format!(
                "invalid maximum active sub-agents: expected an integer from 1 to {HARD_MAX_SUB_AGENT_CONCURRENCY}"
            )
        })?;
        let source = if SUB_AGENT_MAX_CONCURRENCY_PRESETS.contains(&parsed) {
            SubAgentConcurrencySource::Preset
        } else {
            SubAgentConcurrencySource::Custom
        };
        Self::new(parsed, source)
    }
}

impl TryFrom<SubAgentMaxConcurrencyWire> for SubAgentMaxConcurrency {
    type Error = String;

    fn try_from(value: SubAgentMaxConcurrencyWire) -> Result<Self, Self::Error> {
        Self::new(value.value, value.source)
    }
}

impl From<SubAgentMaxConcurrency> for SubAgentMaxConcurrencyWire {
    fn from(value: SubAgentMaxConcurrency) -> Self {
        Self {
            value: value.get(),
            source: value.source(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubAgentReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
}

impl SubAgentReasoningEffort {
    pub const ALL: [Self; 7] = [
        Self::None,
        Self::Minimal,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::XHigh,
        Self::Max,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

impl FromStr for SubAgentReasoningEffort {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::XHigh),
            "max" => Ok(Self::Max),
            other => Err(format!(
                "invalid sub-agent reasoning effort: expected none, minimal, low, medium, high, xhigh, or max, got {other:?}"
            )),
        }
    }
}

pub type SubAgentModelReasoningEffort = SubAgentReasoningEffort;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SubAgentConfig {
    pub provider: ProviderId,
    pub model: Option<String>,
    pub model_reasoning_effort: Option<SubAgentReasoningEffort>,
    pub url: Option<String>,
    #[serde(default)]
    pub max_concurrency: SubAgentMaxConcurrency,
}

impl fmt::Debug for SubAgentConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SubAgentConfig")
            .field("provider", &self.provider)
            .field("model_configured", &self.model.is_some())
            .field("model_reasoning_effort", &self.model_reasoning_effort)
            .field("url_configured", &self.url.is_some())
            .field("max_concurrency", &self.max_concurrency)
            .finish()
    }
}

impl Default for SubAgentConfig {
    fn default() -> Self {
        Self {
            provider: SUPER_SUB_AGENT_DEFAULT_PROVIDER,
            model: None,
            model_reasoning_effort: None,
            url: None,
            max_concurrency: SubAgentMaxConcurrency::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubAgentPreference {
    Unspecified,
    Enabled(SubAgentConfig),
    Disabled,
}

pub type SuperSubAgentConfig = SubAgentConfig;
pub type SuperSubAgentPreference = SubAgentPreference;
pub type SuperSubAgentReasoningEffort = SubAgentReasoningEffort;

#[derive(Clone, PartialEq, Eq)]
pub enum SuperLaunchTarget {
    Fresh,
    Exec,
    Resume { session_id: String },
}

pub type SubAgentLaunchTarget = SuperLaunchTarget;

impl fmt::Debug for SuperLaunchTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SuperLaunchTarget")
            .field(&self.redacted_label())
            .finish()
    }
}

impl SuperLaunchTarget {
    pub fn redacted_label(&self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Exec => "exec",
            Self::Resume { .. } => "resume <SESSION_UUID>",
        }
    }
}

pub fn parse_sub_agent_provider(value: &str) -> Result<ProviderId, String> {
    ProviderId::parse(value).ok_or_else(|| {
        let supported = prodex_provider_core::PROVIDER_IMPLEMENTATION_ORDER
            .iter()
            .map(|provider| provider.label())
            .collect::<Vec<_>>()
            .join(", ");
        format!("invalid --sub-agent-provider: supported values are {supported}, got {value:?}")
    })
}

pub fn parse_sub_agent_model(value: &str) -> Result<String, String> {
    (!value.trim().is_empty())
        .then(|| value.to_owned())
        .ok_or_else(|| "--sub-agent-model must be nonempty".to_string())
}

pub fn parse_sub_agent_reasoning_effort(value: &str) -> Result<SubAgentReasoningEffort, String> {
    value.parse()
}

pub fn parse_sub_agent_url(value: &str) -> Result<String, String> {
    crate::runtime_args::parse_credential_free_http_url(value, "--sub-agent-url")?;
    Ok(value.to_owned())
}

pub fn parse_sub_agent_max_concurrency(value: &str) -> Result<SubAgentMaxConcurrency, String> {
    value.parse()
}

impl fmt::Display for SubAgentReasoningEffort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;

    #[test]
    fn maximum_active_sub_agents_are_typed_and_bounded() {
        assert_eq!(SubAgentMaxConcurrency::default().get(), 4);
        for value in ["default", "4", "8", "16", "32", "1", "23", "64"] {
            assert!(value.parse::<SubAgentMaxConcurrency>().is_ok(), "{value}");
        }
        for value in [
            "",
            "   ",
            "0",
            "65",
            "-1",
            "1.5",
            "1e2",
            "999999999999999999999",
        ] {
            assert!(value.parse::<SubAgentMaxConcurrency>().is_err(), "{value}");
        }
        assert_eq!(
            "4".parse::<SubAgentMaxConcurrency>().unwrap().source(),
            SubAgentConcurrencySource::Preset
        );
        assert_eq!(
            "23".parse::<SubAgentMaxConcurrency>().unwrap().source(),
            SubAgentConcurrencySource::Custom
        );
    }
}
