use super::{ProviderCatalogEntry, provider_catalog_entries_for};
use crate::{ProviderId, ProviderReasoningEffort, provider_runtime_metadata};
use std::fmt;

const ALL_REASONING_EFFORT_LABELS: [&str; 8] = [
    "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
];

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
    Some(match effort {
        ProviderReasoningEffort::None => "none",
        ProviderReasoningEffort::Minimal => "minimal",
        ProviderReasoningEffort::Low => "low",
        ProviderReasoningEffort::Medium => "medium",
        ProviderReasoningEffort::High => "high",
        ProviderReasoningEffort::XHigh => "xhigh",
        ProviderReasoningEffort::Max => "max",
        ProviderReasoningEffort::Ultra => "ultra",
        ProviderReasoningEffort::Unknown => return None,
    })
}

pub(crate) fn reasoning_catalog_data(
    provider: ProviderId,
) -> (
    Vec<&'static ProviderCatalogEntry>,
    Vec<Vec<&'static str>>,
    Vec<Vec<&'static str>>,
) {
    let entries = provider_catalog_entries_for(provider);
    let efforts = entries
        .iter()
        .map(|entry| {
            entry
                .supported_reasoning_efforts
                .as_deref()
                .map(|values| {
                    values
                        .iter()
                        .copied()
                        .filter_map(reasoning_effort_label)
                        .collect()
                })
                .unwrap_or_else(|| ALL_REASONING_EFFORT_LABELS.to_vec())
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
    let fallback_model =
        provider_runtime_metadata(provider).map(|metadata| metadata.default_model);

    #[cfg(feature = "mojo")]
    let plan = {
        let models = entries
            .iter()
            .zip(&aliases)
            .zip(&efforts)
            .map(
                |((entry, aliases), efforts)| prodex_mojo_core::rich::CatalogReasoningModel {
                    id: entry.id.as_str(),
                    aliases,
                    efforts,
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

    #[cfg(not(feature = "mojo"))]
    let _ = &aliases;

    #[cfg(not(feature = "mojo"))]
    let plan = provider_model_reasoning_resolution_rust(
        &entries,
        &efforts,
        model.filter(|model| !model.trim().is_empty()),
        fallback_model,
        requested_effort.filter(|effort| !effort.trim().is_empty()),
    )?;

    let supported_reasoning_efforts = plan
        .supported_efforts
        .iter()
        .map(|effort| ProviderReasoningEffort::parse(effort))
        .filter(|effort| *effort != ProviderReasoningEffort::Unknown)
        .collect();
    Ok(ProviderModelReasoningResolution {
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
    })
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn provider_model_reasoning_resolution_rust(
    entries: &[&ProviderCatalogEntry],
    efforts: &[Vec<&str>],
    model: Option<&str>,
    fallback_model: Option<&str>,
    requested_effort: Option<&str>,
) -> Result<ProviderReasoningPlan, ProviderModelReasoningError> {
    let model_index = model
        .and_then(|model| find_catalog_entry_rust(entries, model))
        .or_else(|| fallback_model.and_then(|model| find_catalog_entry_rust(entries, model)));
    let Some(model_index) = model_index else {
        return Ok(ProviderReasoningPlan {
            model_index: None,
            supported_efforts: Vec::new(),
            selected_effort: None,
            default_effort: None,
        });
    };
    let supported_efforts = efforts[model_index]
        .iter()
        .map(|effort| (*effort).to_string())
        .collect::<Vec<_>>();
    let default_effort = entries[model_index]
        .default_reasoning_effort
        .and_then(reasoning_effort_label)
        .map(str::to_string)
        .or_else(|| supported_efforts.first().cloned());
    let selected_effort = if let Some(requested) = requested_effort {
        Some(
            supported_efforts
                .iter()
                .find(|effort| effort.eq_ignore_ascii_case(requested.trim()))
                .cloned()
                .ok_or(ProviderModelReasoningError::UnsupportedEffort)?,
        )
    } else {
        default_effort.clone()
    };
    Ok(ProviderReasoningPlan {
        model_index: Some(model_index),
        supported_efforts,
        selected_effort,
        default_effort,
    })
}

#[cfg(any(not(feature = "mojo"), test))]
fn find_catalog_entry_rust(entries: &[&ProviderCatalogEntry], model: &str) -> Option<usize> {
    let model = model.trim();
    entries.iter().position(|entry| {
        entry.id.eq_ignore_ascii_case(model)
            || entry
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(model))
    })
}
