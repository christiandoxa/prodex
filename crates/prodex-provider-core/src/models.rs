use super::{ProviderId, ProviderModelCost, ProviderModelSpec};

mod anthropic;
mod copilot;
mod deepseek;
mod gemini;
mod kiro;
mod local;
mod openai;

const OPENAI_CONTEXT_WINDOW_TOKENS: u64 = 400_000;
const OPENAI_CODEX_SPARK_CONTEXT_WINDOW_TOKENS: u64 = 128_000;
const ANTHROPIC_CONTEXT_WINDOW_TOKENS: u64 = 200_000;
const COPILOT_OPENAI_CONTEXT_WINDOW_TOKENS: u64 = 400_000;
const COPILOT_EXTENDED_CONTEXT_WINDOW_TOKENS: u64 = 1_000_000;
const COPILOT_ANTHROPIC_CONTEXT_WINDOW_TOKENS: u64 = 200_000;
const COPILOT_GEMINI_CONTEXT_WINDOW_TOKENS: u64 = 1_048_576;
const DEEPSEEK_CONTEXT_WINDOW_TOKENS: u64 = 128_000;
const GEMINI_CONTEXT_WINDOW_TOKENS: u64 = 1_048_576;
const KIRO_CONTEXT_WINDOW_TOKENS: u64 = 200_000;

macro_rules! model {
    ($provider:expr, $owned_by:expr, $id:expr, $display:expr, $description:expr, $ctx:expr, $in_cost:expr, $out_cost:expr, $endpoints:expr, [$($alias:expr),* $(,)?]) => {
        ProviderModelSpec {
            id: $id,
            display_name: $display,
            description: $description,
            provider: $provider,
            owned_by: $owned_by,
            context_window_tokens: $ctx,
            input_cost_per_million_microusd: $in_cost,
            output_cost_per_million_microusd: $out_cost,
            endpoints: $endpoints,
            aliases: &[$($alias),*],
        }
    };
}
pub(super) use model;

pub fn provider_model_catalog(provider: ProviderId) -> &'static [ProviderModelSpec] {
    match provider {
        ProviderId::OpenAi => openai::MODELS,
        ProviderId::Anthropic => anthropic::MODELS,
        ProviderId::Copilot => copilot::MODELS,
        ProviderId::DeepSeek => deepseek::MODELS,
        ProviderId::Gemini => gemini::MODELS,
        ProviderId::Kiro => kiro::MODELS,
        ProviderId::Local => local::MODELS,
    }
}

pub fn provider_model_spec(
    provider: ProviderId,
    model: &str,
) -> Option<&'static ProviderModelSpec> {
    let model = model.trim();
    provider_model_catalog(provider)
        .iter()
        .find(|spec| spec.matches_id_or_alias(model))
}

pub fn provider_model_cost(provider: ProviderId, model: &str) -> ProviderModelCost {
    provider_model_spec(provider, model)
        .map(|spec| spec.cost())
        .unwrap_or_default()
}
