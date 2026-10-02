//! Public provider-core identifiers, endpoint contracts, and body transform wrappers.

use serde::{Deserialize, Serialize};

#[path = "surface/adapter_contract.rs"]
mod adapter_contract;
#[path = "surface/endpoints.rs"]
mod endpoints;
#[path = "surface/models.rs"]
mod models;

pub use self::adapter_contract::{ProviderBodyTransform, ProviderTransformPhase};
pub use self::endpoints::{ALL_PROVIDER_ENDPOINTS, provider_supported_endpoints};
pub use self::models::{ProviderModelCost, ProviderModelSpec};

pub const PRODEX_ANTHROPIC_DEFAULT_MODEL: &str = "claude-sonnet-5-5";
pub const PRODEX_COPILOT_DEFAULT_MODEL: &str = "gpt-6-astra";
pub const PRODEX_GEMINI_DEFAULT_MODEL: &str = "auto";
pub const PRODEX_GEMINI_CHAT_COMPRESSION_MODEL: &str = "chat-compression-default";
pub const PRODEX_KIRO_DEFAULT_MODEL: &str = "auto";

#[repr(i64)]
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
    pub fn label(self) -> &'static str {
        prodex_mojo_core::provider_constraints::provider_id_label(self as i64)
            .expect("Mojo provider-id label policy failed")
    }

    pub fn parse(value: &str) -> Option<Self> {
        crate::provider_implementation_registry().resolve_alias(value)
    }
}

#[repr(i64)]
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
    pub fn label(self) -> &'static str {
        prodex_mojo_core::provider_constraints::provider_wire_format_label(self as i64)
            .expect("Mojo provider wire-format label policy failed")
    }
}

#[repr(i64)]
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
    pub fn label(self) -> &'static str {
        prodex_mojo_core::provider_constraints::provider_endpoint_label(self as i64)
            .expect("Mojo provider endpoint label policy failed")
    }
}

#[repr(i64)]
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
    pub fn label(self) -> &'static str {
        prodex_mojo_core::provider_constraints::provider_capability_status_label(self as i64)
            .expect("Mojo provider capability-status label policy failed")
    }
}
