//! Provider-scoped published lifecycle facts, matched by the compiled identity owner.
use super::{ProviderId, provider_catalog_entry};
use prodex_mojo_core::rich::{CatalogModel, resolve_catalog_model};
use serde::Deserialize;
use std::sync::OnceLock;

/// Authentication surface where an upstream model retirement applies.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCatalogScope {
    /// Provider-wide API/agent availability; never infer retirement from age.
    #[default]
    Provider,
    /// OpenAI Codex authenticated with ChatGPT, distinct from API-key access.
    Chatgpt,
}

#[derive(Deserialize)]
struct RetirementRegistry {
    retirements: Vec<Retirement>,
}

#[derive(Deserialize)]
struct Retirement {
    provider: ProviderId,
    scope: ModelCatalogScope,
    models: Vec<String>,
}

fn retirements() -> &'static [Retirement] {
    static REGISTRY: OnceLock<RetirementRegistry> = OnceLock::new();
    &REGISTRY
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../catalog/retired-models.json"))
                .expect("reviewed provider lifecycle registry must be valid")
        })
        .retirements
}

/// Whether published upstream evidence retires this exact ID on the provider.
/// This never applies another provider's retirement to a local/custom endpoint.
pub fn provider_model_is_retired(provider: ProviderId, model: &str) -> bool {
    provider_model_is_retired_in_scope(provider, model, ModelCatalogScope::Provider)
}

/// Match retirement facts with the existing Mojo Unicode-trim/casefold identity
/// contract. ChatGPT-only retirements do not apply to API-key/unknown auth.
pub fn provider_model_is_retired_in_scope(
    provider: ProviderId,
    model: &str,
    scope: ModelCatalogScope,
) -> bool {
    // Preserve the existing authoritative Spark-retirement ABI. Additional
    // reviewed provider facts use the shared compiled exact-identity planner.
    if provider == ProviderId::OpenAi
        && prodex_mojo_core::rich::openai_model_is_retired(model)
            .expect("compiled OpenAI retirement classification failed")
    {
        return true;
    }
    let canonical = provider_catalog_entry(provider, model)
        .map(|entry| entry.id.as_str())
        .unwrap_or(model);
    let entries = retirements()
        .iter()
        .filter(|entry| {
            entry.provider == provider
                && (entry.scope == ModelCatalogScope::Provider || entry.scope == scope)
        })
        .flat_map(|entry| entry.models.iter())
        .map(|id| CatalogModel { id, aliases: &[] })
        .collect::<Vec<_>>();
    resolve_catalog_model(&entries, canonical)
        .expect("compiled provider lifecycle identity matching failed")
        .is_some()
}

/// Treat a successfully discovered account catalog as the source of selectable
/// models instead of unioning it with unrelated offline suggestions. Unknown
/// explicitly remembered custom IDs remain usable; absent bundled IDs do not.
pub fn resolve_provider_snapshot_model_choices(
    provider: ProviderId,
    models: &[String],
    current_model: Option<&str>,
) -> Vec<super::ProviderModelChoice> {
    use super::ProviderModelChoice;
    use prodex_mojo_core::rich::{CatalogChoice, plan_catalog_choices};
    let entries = models
        .iter()
        .filter(|model| !provider_model_is_retired(provider, model))
        .collect::<Vec<_>>();
    let aliases = entries
        .iter()
        .map(|id| {
            provider_catalog_entry(provider, id)
                .map(|entry| entry.aliases.iter().map(String::as_str).collect::<Vec<_>>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let catalog = entries
        .iter()
        .zip(&aliases)
        .map(|(id, aliases)| CatalogModel { id, aliases })
        .collect::<Vec<_>>();
    let current = current_model.filter(|model| {
        !provider_model_is_retired(provider, model)
            && (provider_catalog_entry(provider, model).is_none()
                || resolve_catalog_model(&catalog, model)
                    .expect("compiled account catalog identity matching failed")
                    .is_some())
    });
    plan_catalog_choices(&catalog, &[], current)
        .expect("compiled account catalog choice planning failed")
        .into_iter()
        .map(|choice| match choice {
            CatalogChoice::ProviderDefault => ProviderModelChoice::ProviderDefault,
            CatalogChoice::Catalog(index) => ProviderModelChoice::Model(entries[index].to_string()),
            CatalogChoice::Current => {
                ProviderModelChoice::Model(current.expect("planned current model").trim().into())
            }
            CatalogChoice::Custom => ProviderModelChoice::Custom,
            CatalogChoice::Configured(_) => unreachable!("no configured candidates were supplied"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chatgpt_retirements_preserve_api_key_models_and_provider_exceptions() {
        for id in [
            "gpt-5.5",
            "GPT-5.4",
            "gpt-5.4-mini",
            "gpt-5.3-codex",
            "gpt-5.2",
            "opus",
        ] {
            assert!(
                provider_model_is_retired_in_scope(
                    ProviderId::OpenAi,
                    id,
                    ModelCatalogScope::Chatgpt
                ),
                "{id}"
            );
            assert!(!provider_model_is_retired(ProviderId::OpenAi, id), "{id}");
        }
        for (provider, id) in [
            (ProviderId::Copilot, "claude-sonnet-4-6"),
            (ProviderId::Anthropic, "claude-sonnet-4-5-20250929"),
            (ProviderId::Kiro, "claude-opus-4-6"),
            (ProviderId::Gemini, "gemini-3.7-flash"),
            (ProviderId::DeepSeek, "deepseek-v4-flash"),
            (ProviderId::Local, "claude-sonnet-4"),
        ] {
            assert!(
                !provider_model_is_retired(provider, id),
                "{provider:?} {id}"
            );
        }
    }

    #[test]
    fn account_snapshot_does_not_restore_offline_or_remembered_models() {
        use super::super::ProviderModelChoice;
        let snapshot = ["gpt-6-luna".into(), "tenant/custom".into()];
        let choices =
            resolve_provider_snapshot_model_choices(ProviderId::OpenAi, &snapshot, Some("gpt-5.4"));
        assert_eq!(
            choices,
            [
                ProviderModelChoice::ProviderDefault,
                ProviderModelChoice::Model("gpt-6-luna".into()),
                ProviderModelChoice::Model("tenant/custom".into()),
                ProviderModelChoice::Custom
            ]
        );
        let unknown = resolve_provider_snapshot_model_choices(
            ProviderId::OpenAi,
            &snapshot,
            Some("tenant/explicit"),
        );
        assert!(unknown.contains(&ProviderModelChoice::Model("tenant/explicit".into())));
        let alias =
            resolve_provider_snapshot_model_choices(ProviderId::OpenAi, &snapshot, Some("LUNA"));
        assert_eq!(alias, choices);
    }

    #[test]
    fn static_catalog_contains_no_provider_wide_retirements() {
        for entry in super::super::provider_catalog_entries() {
            assert!(
                !provider_model_is_retired(entry.provider, &entry.id),
                "{}:{}",
                entry.provider.label(),
                entry.id
            );
        }
    }

    #[test]
    fn retired_ids_match_exactly_and_never_remove_custom_namespaces() {
        for entry in retirements() {
            for id in &entry.models {
                let padded = format!("\u{3000}{}\t", id.to_uppercase());
                assert!(provider_model_is_retired_in_scope(
                    entry.provider,
                    &padded,
                    entry.scope
                ));
                for custom in [format!("custom/{id}"), format!("{id}-custom")] {
                    assert!(!provider_model_is_retired_in_scope(
                        entry.provider,
                        &custom,
                        entry.scope
                    ));
                }
            }
        }
    }
}
