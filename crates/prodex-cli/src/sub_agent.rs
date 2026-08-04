use prodex_provider_core::ProviderId;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub const SUPER_SUB_AGENT_DEFAULT_PROVIDER: ProviderId = ProviderId::OpenAi;

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
}

impl fmt::Debug for SubAgentConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SubAgentConfig")
            .field("provider", &self.provider)
            .field("model_configured", &self.model.is_some())
            .field("model_reasoning_effort", &self.model_reasoning_effort)
            .field("url_configured", &self.url.is_some())
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

impl fmt::Display for SubAgentReasoningEffort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
