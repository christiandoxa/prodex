use super::provider_bridge::RuntimeProviderBridgeKind;

#[derive(Clone, Copy)]
pub(super) struct RuntimeProviderModelSpec {
    pub(super) id: &'static str,
    pub(super) owned_by: &'static str,
}

const RUNTIME_PROVIDER_GEMINI_MODELS: &[RuntimeProviderModelSpec] = &[
    RuntimeProviderModelSpec {
        id: "auto",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "auto-gemini-3",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "auto-gemini-2.5",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "pro",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "flash",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "flash-lite",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-3.1-pro-preview",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-3-pro-preview",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-3.1-pro-preview-customtools",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-3-flash-preview",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-3-flash",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-3.5-flash",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-2.5-pro",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-2.5-flash",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-3.1-flash-lite",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemini-2.5-flash-lite",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemma-4-31b-it",
        owned_by: "google",
    },
    RuntimeProviderModelSpec {
        id: "gemma-4-26b-a4b-it",
        owned_by: "google",
    },
];

const RUNTIME_PROVIDER_DEEPSEEK_MODELS: &[RuntimeProviderModelSpec] = &[
    RuntimeProviderModelSpec {
        id: "deepseek-v4-pro",
        owned_by: "deepseek",
    },
    RuntimeProviderModelSpec {
        id: "deepseek-v4-flash",
        owned_by: "deepseek",
    },
    RuntimeProviderModelSpec {
        id: "deepseek-chat",
        owned_by: "deepseek",
    },
    RuntimeProviderModelSpec {
        id: "deepseek-reasoner",
        owned_by: "deepseek",
    },
];

const RUNTIME_PROVIDER_ANTHROPIC_MODELS: &[RuntimeProviderModelSpec] = &[
    RuntimeProviderModelSpec {
        id: "auto",
        owned_by: "anthropic",
    },
    RuntimeProviderModelSpec {
        id: "opus",
        owned_by: "anthropic",
    },
    RuntimeProviderModelSpec {
        id: "sonnet",
        owned_by: "anthropic",
    },
    RuntimeProviderModelSpec {
        id: "haiku",
        owned_by: "anthropic",
    },
    RuntimeProviderModelSpec {
        id: "claude-opus-4-8",
        owned_by: "anthropic",
    },
    RuntimeProviderModelSpec {
        id: "claude-sonnet-4-6",
        owned_by: "anthropic",
    },
    RuntimeProviderModelSpec {
        id: "claude-haiku-4-5",
        owned_by: "anthropic",
    },
    RuntimeProviderModelSpec {
        id: "claude-opus-4-6",
        owned_by: "anthropic",
    },
    RuntimeProviderModelSpec {
        id: "claude-opus-4-20250514",
        owned_by: "anthropic",
    },
];

const RUNTIME_PROVIDER_COPILOT_MODELS: &[RuntimeProviderModelSpec] = &[
    RuntimeProviderModelSpec {
        id: "auto",
        owned_by: "github-copilot",
    },
    RuntimeProviderModelSpec {
        id: "codex",
        owned_by: "github-copilot",
    },
    RuntimeProviderModelSpec {
        id: "gpt-5.1-codex",
        owned_by: "github-copilot",
    },
    RuntimeProviderModelSpec {
        id: "gpt-5.4",
        owned_by: "github-copilot",
    },
    RuntimeProviderModelSpec {
        id: "gpt-5.3-codex",
        owned_by: "github-copilot",
    },
    RuntimeProviderModelSpec {
        id: "claude-sonnet-4-6",
        owned_by: "github-copilot",
    },
    RuntimeProviderModelSpec {
        id: "gemini-3.1-pro-preview",
        owned_by: "github-copilot",
    },
];

const RUNTIME_PROVIDER_OPENAI_MODELS: &[RuntimeProviderModelSpec] = &[];

pub(super) fn runtime_provider_model_catalog(
    kind: RuntimeProviderBridgeKind,
) -> &'static [RuntimeProviderModelSpec] {
    match kind {
        RuntimeProviderBridgeKind::Anthropic => RUNTIME_PROVIDER_ANTHROPIC_MODELS,
        RuntimeProviderBridgeKind::Copilot => RUNTIME_PROVIDER_COPILOT_MODELS,
        RuntimeProviderBridgeKind::OpenAiResponses => RUNTIME_PROVIDER_OPENAI_MODELS,
        RuntimeProviderBridgeKind::DeepSeek => RUNTIME_PROVIDER_DEEPSEEK_MODELS,
        RuntimeProviderBridgeKind::Gemini => RUNTIME_PROVIDER_GEMINI_MODELS,
    }
}
