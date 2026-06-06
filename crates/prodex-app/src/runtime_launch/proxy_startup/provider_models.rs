use super::provider_bridge::RuntimeProviderBridgeKind;
use prodex_runtime_gemini::{gemini_model_catalog, gemini_model_spec};

#[derive(Clone, Copy)]
pub(super) struct RuntimeProviderModelSpec {
    pub(super) id: &'static str,
    pub(super) owned_by: &'static str,
}

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
        RuntimeProviderBridgeKind::Gemini => RUNTIME_PROVIDER_OPENAI_MODELS,
    }
}

pub(super) fn runtime_provider_model_catalog_json(
    kind: RuntimeProviderBridgeKind,
) -> Vec<serde_json::Value> {
    match kind {
        RuntimeProviderBridgeKind::Gemini => gemini_model_catalog()
            .iter()
            .map(|model| runtime_provider_model_json(model.id, model.owned_by))
            .collect(),
        _ => runtime_provider_model_catalog(kind)
            .iter()
            .map(|model| runtime_provider_model_json(model.id, model.owned_by))
            .collect(),
    }
}

pub(super) fn runtime_provider_model_json_for(
    kind: RuntimeProviderBridgeKind,
    model_id: &str,
) -> Option<serde_json::Value> {
    match kind {
        RuntimeProviderBridgeKind::Gemini => gemini_model_spec(model_id)
            .map(|model| runtime_provider_model_json(model.id, model.owned_by)),
        _ => runtime_provider_model_catalog(kind)
            .iter()
            .find(|model| model.id == model_id)
            .map(|model| runtime_provider_model_json(model.id, model.owned_by)),
    }
}

fn runtime_provider_model_json(id: &str, owned_by: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "object": "model",
        "owned_by": owned_by,
    })
}
