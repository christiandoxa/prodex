use super::{ProviderCatalogEntry, provider_catalog_entries_for};
use crate::{ProviderId, ProviderReasoningEffort, provider_runtime_metadata};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderModelReasoningResolution {
    pub model_index: Option<usize>,
    pub supported_reasoning_efforts: Vec<ProviderReasoningEffort>,
    pub default_reasoning_effort: Option<ProviderReasoningEffort>,
    pub selected_reasoning_effort: Option<ProviderReasoningEffort>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderModelReasoningError {
    InvalidCatalog,
    UnsupportedEffort,
}

impl fmt::Display for ProviderModelReasoningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCatalog => "provider model reasoning catalog is invalid",
            Self::UnsupportedEffort => "reasoning effort is unsupported for the selected model",
        })
    }
}

impl std::error::Error for ProviderModelReasoningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderReasoningPlan {
    pub(crate) model_index: Option<usize>,
    pub(crate) supported_efforts: Vec<String>,
    pub(crate) selected_effort: Option<String>,
    pub(crate) default_effort: Option<String>,
}

pub(crate) fn reasoning_effort_label(effort: ProviderReasoningEffort) -> Option<&'static str> {
    effort.label()
}

type ReasoningCatalogData = (
    Vec<&'static ProviderCatalogEntry>,
    Vec<Option<Vec<&'static str>>>,
    Vec<Vec<&'static str>>,
);

pub(crate) fn reasoning_catalog_data(provider: ProviderId) -> ReasoningCatalogData {
    let entries = provider_catalog_entries_for(provider);
    let efforts = entries
        .iter()
        .map(|entry| {
            entry.supported_reasoning_efforts.as_deref().map(|values| {
                values
                    .iter()
                    .copied()
                    .filter_map(reasoning_effort_label)
                    .collect()
            })
        })
        .collect::<Vec<_>>();
    let aliases = entries
        .iter()
        .map(|entry| entry.aliases.iter().map(String::as_str).collect())
        .collect();
    (entries, efforts, aliases)
}

pub fn provider_model_reasoning_resolution(
    provider: ProviderId,
    model: Option<&str>,
    requested_effort: Option<&str>,
) -> Result<ProviderModelReasoningResolution, ProviderModelReasoningError> {
    let (entries, efforts, aliases) = reasoning_catalog_data(provider);
    let fallback_model = provider_runtime_metadata(provider).map(|metadata| metadata.default_model);

    let plan = {
        let models = entries
            .iter()
            .zip(&aliases)
            .zip(&efforts)
            .map(
                |((entry, aliases), efforts)| prodex_mojo_core::rich::CatalogReasoningModel {
                    id: entry.id.as_str(),
                    aliases,
                    efforts: efforts.as_deref(),
                    default_effort: entry
                        .default_reasoning_effort
                        .and_then(reasoning_effort_label),
                },
            )
            .collect::<Vec<_>>();
        let plan = prodex_mojo_core::rich::resolve_catalog_reasoning(
            &models,
            model.filter(|model| !model.trim().is_empty()),
            fallback_model,
            requested_effort.filter(|effort| !effort.trim().is_empty()),
        )
        .map_err(|error| match error {
            prodex_mojo_core::MojoError::Structured(issue) if issue.kind == 5 => {
                ProviderModelReasoningError::UnsupportedEffort
            }
            _ => ProviderModelReasoningError::InvalidCatalog,
        })?;
        ProviderReasoningPlan {
            model_index: plan.model_index,
            supported_efforts: plan.supported_efforts,
            selected_effort: plan.selected_effort,
            default_effort: plan.default_effort,
        }
    };

    Ok(reasoning_resolution(plan))
}

fn reasoning_resolution(plan: ProviderReasoningPlan) -> ProviderModelReasoningResolution {
    let supported_reasoning_efforts = plan
        .supported_efforts
        .iter()
        .map(|effort| ProviderReasoningEffort::parse(effort))
        .filter(|effort| *effort != ProviderReasoningEffort::Unknown)
        .collect();
    ProviderModelReasoningResolution {
        model_index: plan.model_index,
        supported_reasoning_efforts,
        default_reasoning_effort: plan
            .default_effort
            .as_deref()
            .map(ProviderReasoningEffort::parse)
            .filter(|effort| *effort != ProviderReasoningEffort::Unknown),
        selected_reasoning_effort: plan
            .selected_effort
            .as_deref()
            .map(ProviderReasoningEffort::parse)
            .filter(|effort| *effort != ProviderReasoningEffort::Unknown),
    }
}

/// Resolve a model using available account metadata before static API defaults.
/// The caller supplies its validated selectable catalog; missing effort metadata
/// preserves the offline resolution. Identity and effort validation stay in Mojo.
pub fn provider_model_reasoning_resolution_with_catalog(
    provider: ProviderId,
    model: Option<&str>,
    requested_effort: Option<&str>,
    available_models: &[serde_json::Value],
) -> Result<ProviderModelReasoningResolution, ProviderModelReasoningError> {
    use prodex_mojo_core::rich::{CatalogModel, CatalogReasoningModel};
    let Some(model) = model.filter(|model| !model.trim().is_empty()) else {
        return provider_model_reasoning_resolution(provider, model, requested_effort);
    };
    let canonical = super::provider_catalog_entry(provider, model)
        .map(|entry| entry.id.as_str())
        .unwrap_or(model);
    let projected = super::project_main_catalog_models(available_models)
        .map_err(|_| ProviderModelReasoningError::InvalidCatalog)?;
    let aliases = projected
        .iter()
        .map(|entry| entry.aliases.iter().map(String::as_str).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let identities = projected
        .iter()
        .zip(&aliases)
        .map(|(entry, aliases)| CatalogModel {
            id: &entry.id,
            aliases,
        })
        .collect::<Vec<_>>();
    let index = prodex_mojo_core::rich::resolve_catalog_model(&identities, canonical)
        .map_err(|_| ProviderModelReasoningError::InvalidCatalog)?;
    let Some(index) = index else {
        return provider_model_reasoning_resolution(provider, Some(model), requested_effort);
    };
    let entry = &projected[index];
    if entry.efforts.is_empty() {
        return provider_model_reasoning_resolution(provider, Some(model), requested_effort);
    }
    let efforts = entry.efforts.iter().map(String::as_str).collect::<Vec<_>>();
    let plan = prodex_mojo_core::rich::resolve_catalog_reasoning(
        &[CatalogReasoningModel {
            id: &entry.id,
            aliases: &aliases[index],
            efforts: Some(&efforts),
            default_effort: entry.default_effort.as_deref(),
        }],
        Some(canonical),
        None,
        requested_effort,
    )
    .map_err(|error| match error {
        prodex_mojo_core::MojoError::Structured(issue) if issue.kind == 5 => {
            ProviderModelReasoningError::UnsupportedEffort
        }
        _ => ProviderModelReasoningError::InvalidCatalog,
    })?;
    Ok(reasoning_resolution(ProviderReasoningPlan {
        model_index: Some(index),
        supported_efforts: plan.supported_efforts,
        selected_effort: plan.selected_effort,
        default_effort: plan.default_effort,
    }))
}
