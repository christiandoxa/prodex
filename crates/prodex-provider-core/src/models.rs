use super::{ProviderId, ProviderModelCost, ProviderModelSpec};
use std::sync::LazyLock;

const PROVIDERS: [ProviderId; 7] = [
    ProviderId::OpenAi,
    ProviderId::Anthropic,
    ProviderId::Copilot,
    ProviderId::DeepSeek,
    ProviderId::Gemini,
    ProviderId::Kiro,
    ProviderId::Local,
];

fn provider_index(provider: ProviderId) -> usize {
    PROVIDERS
        .iter()
        .position(|candidate| *candidate == provider)
        .expect("built-in provider id")
}

fn build_catalog(provider: ProviderId) -> Box<[ProviderModelSpec]> {
    crate::catalog::provider_catalog_entries_for(provider)
        .into_iter()
        .map(|entry| {
            let aliases = entry
                .aliases
                .iter()
                .map(|alias| alias.as_str())
                .collect::<Vec<_>>()
                .into_boxed_slice();
            ProviderModelSpec {
                id: entry.id.as_str(),
                display_name: entry.display_name.as_str(),
                description: entry.description.as_str(),
                provider: entry.provider,
                owned_by: entry.owned_by.as_str(),
                context_window_tokens: entry.context_window_tokens,
                input_cost_per_million_microusd: entry.input_cost_per_million_microusd,
                output_cost_per_million_microusd: entry.output_cost_per_million_microusd,
                endpoints: entry.supported_endpoints.as_slice(),
                aliases: Box::leak(aliases),
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

static MODEL_CATALOGS: LazyLock<[Box<[ProviderModelSpec]>; 7]> =
    LazyLock::new(|| std::array::from_fn(|index| build_catalog(PROVIDERS[index])));

pub(crate) fn builtin_model_catalog(provider: ProviderId) -> &'static [ProviderModelSpec] {
    MODEL_CATALOGS[provider_index(provider)].as_ref()
}

pub fn provider_model_catalog(provider: ProviderId) -> &'static [ProviderModelSpec] {
    builtin_model_catalog(provider)
}

pub fn provider_model_spec(
    provider: ProviderId,
    model: &str,
) -> Option<&'static ProviderModelSpec> {
    let models = provider_model_catalog(provider);
    let aliases = models
        .iter()
        .map(|spec| spec.aliases.to_vec())
        .collect::<Vec<_>>();
    let catalog = models
        .iter()
        .zip(&aliases)
        .map(|(spec, aliases)| prodex_mojo_core::rich::CatalogModel {
            id: spec.id,
            aliases,
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::rich::resolve_catalog_model(&catalog, model.trim())
        .expect("Mojo model catalog lookup returned an invalid structured result")
        .and_then(|index| models.get(index))
}

pub fn provider_model_cost(provider: ProviderId, model: &str) -> ProviderModelCost {
    provider_model_spec(provider, model)
        .map(|spec| spec.cost())
        .unwrap_or_default()
}
