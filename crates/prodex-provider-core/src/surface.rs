//! Public provider-core identifiers, endpoint contracts, and body transform wrappers.

use serde::{Deserialize, Serialize};

#[path = "surface/adapter_contract.rs"]
mod adapter_contract;
#[path = "surface/endpoints.rs"]
mod endpoints;
#[path = "surface/models.rs"]
mod models;

pub use self::adapter_contract::{
    ProviderAdapterContract, ProviderBodyTransform, ProviderTransformPhase,
};
pub use self::endpoints::{ALL_PROVIDER_ENDPOINTS, provider_supported_endpoints};
pub(crate) use self::endpoints::{
    COPILOT_TEXT_ENDPOINTS, CORE_TEXT_ENDPOINTS, GEMINI_ENDPOINTS, KIRO_ENDPOINTS, OPENAI_ENDPOINTS,
};
pub use self::models::{ProviderModelCost, ProviderModelSpec};

pub const PRODEX_ANTHROPIC_DEFAULT_MODEL: &str = "claude-sonnet-4-6";
pub const PRODEX_COPILOT_DEFAULT_MODEL: &str = "gpt-5.3-codex";
pub const PRODEX_GEMINI_DEFAULT_MODEL: &str = "auto";
pub const PRODEX_GEMINI_CHAT_COMPRESSION_MODEL: &str = "chat-compression-default";
pub const PRODEX_KIRO_DEFAULT_MODEL: &str = "claude-sonnet-4";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ProviderId {
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "copilot")]
    Copilot,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "kiro")]
    Kiro,
    #[serde(rename = "local")]
    Local,
}

impl ProviderId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Copilot => "copilot",
            Self::DeepSeek => "deepseek",
            Self::Gemini => "gemini",
            Self::Kiro => "kiro",
            Self::Local => "local",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" | "openai-responses" | "openai_compatible" | "openai-compatible" => {
                Some(Self::OpenAi)
            }
            "anthropic" | "claude" => Some(Self::Anthropic),
            "copilot" | "github-copilot" | "github_copilot" => Some(Self::Copilot),
            "deepseek" => Some(Self::DeepSeek),
            "gemini" | "google" => Some(Self::Gemini),
            "kiro" => Some(Self::Kiro),
            "local" | "local-openai" | "local_openai" => Some(Self::Local),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderWireFormat {
    OpenAiResponses,
    OpenAiChatCompletions,
    AnthropicMessages,
    GeminiGenerateContent,
    Passthrough,
}

impl ProviderWireFormat {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai-responses",
            Self::OpenAiChatCompletions => "openai-chat-completions",
            Self::AnthropicMessages => "anthropic-messages",
            Self::GeminiGenerateContent => "gemini-generate-content",
            Self::Passthrough => "passthrough",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderEndpoint {
    Responses,
    #[serde(rename = "responses/compact")]
    ResponsesCompact,
    ChatCompletions,
    Messages,
    Models,
    Embeddings,
    Images,
    Audio,
    Batches,
    Rerank,
    A2a,
}

impl ProviderEndpoint {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ResponsesCompact => "responses/compact",
            Self::ChatCompletions => "chat-completions",
            Self::Messages => "messages",
            Self::Models => "models",
            Self::Embeddings => "embeddings",
            Self::Images => "images",
            Self::Audio => "audio",
            Self::Batches => "batches",
            Self::Rerank => "rerank",
            Self::A2a => "a2a",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderCapabilityStatus {
    Native,
    Translated,
    Passthrough,
    Emulated,
    Partial,
    Unsupported,
    Untested,
}

impl ProviderCapabilityStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Translated => "translated",
            Self::Passthrough => "passthrough",
            Self::Emulated => "emulated",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
            Self::Untested => "untested",
        }
    }
}
