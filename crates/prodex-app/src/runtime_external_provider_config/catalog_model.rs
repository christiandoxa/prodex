use super::{
    COPILOT_RUNTIME_MODEL_CATALOG_FILE, ExternalCatalogProvider, read_provider_model_catalog_text,
};
use crate::profile_commands::KIRO_MODEL_CATALOG_FILE;
use anyhow::{Context, Result, bail};
use prodex_provider_core::{ProviderReasoningEffort, provider_catalog_entry};
use serde_json::{Value, json};
#[cfg(any(not(feature = "mojo-core"), test))]
use std::collections::BTreeSet;
use std::path::Path;

const DEFAULT_REASONING_EFFORTS: [ProviderReasoningEffort; 4] = [
    ProviderReasoningEffort::Low,
    ProviderReasoningEffort::Medium,
    ProviderReasoningEffort::High,
    ProviderReasoningEffort::XHigh,
];

pub(super) fn read_external_model_catalog(
    provider: ExternalCatalogProvider,
    contents: &str,
) -> Result<Value> {
    if matches!(provider, ExternalCatalogProvider::Kiro) {
        return Ok(json!({
            "models": crate::profile_commands::parse_kiro_model_catalog_text(contents)
                .context("failed to parse Kiro model catalog")?,
        }));
    }
    serde_json::from_str(contents).context("failed to parse provider model catalog")
}

pub(super) fn external_catalog_model(
    provider: ExternalCatalogProvider,
    slug: &str,
    display_name: &str,
    description: &str,
    priority: usize,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> serde_json::Value {
    let kiro = matches!(provider, ExternalCatalogProvider::Kiro);
    let catalog_entry = provider_catalog_entry(provider.provider_id(), slug);
    let reasoning_efforts = catalog_entry
        .and_then(|entry| entry.supported_reasoning_efforts.as_deref())
        .unwrap_or(&DEFAULT_REASONING_EFFORTS);
    let supported_reasoning_levels = reasoning_efforts
        .iter()
        .filter_map(|effort| {
            let description = match effort {
                ProviderReasoningEffort::None => "No reasoning effort",
                ProviderReasoningEffort::Minimal => "Minimal reasoning effort",
                ProviderReasoningEffort::Low => "Low reasoning effort",
                ProviderReasoningEffort::Medium => "Medium reasoning effort",
                ProviderReasoningEffort::High => "High reasoning effort",
                ProviderReasoningEffort::XHigh => "Extra-high reasoning effort",
                ProviderReasoningEffort::Max => "Max reasoning effort",
                ProviderReasoningEffort::Ultra => "Ultra reasoning effort",
                ProviderReasoningEffort::Unknown => return None,
            };
            Some(json!({ "effort": effort, "description": description }))
        })
        .collect::<Vec<_>>();
    let default_reasoning_level = catalog_entry
        .and_then(|entry| entry.default_reasoning_effort)
        .filter(|effort| *effort != ProviderReasoningEffort::Unknown)
        .unwrap_or(ProviderReasoningEffort::High);
    json!({
        "slug": slug,
        "display_name": display_name,
        "description": description,
        "default_reasoning_level": default_reasoning_level,
        "supported_reasoning_levels": supported_reasoning_levels,
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": priority,
        "additional_speed_tiers": [],
        "service_tiers": [],
        "default_service_tier": null,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": "",
        "supports_reasoning_summaries": !kiro,
        "supports_reasoning_summary_parameter": !kiro,
        "default_reasoning_summary": "none",
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": "freeform",
        "web_search_tool_type": "text",
        "truncation_policy": {
            "mode": "tokens",
            "limit": 10000
        },
        "supports_parallel_tool_calls": true,
        "supports_image_detail_original": false,
        "context_window": context_window,
        "max_context_window": context_window,
        "auto_compact_token_limit": auto_compact_token_limit,
        "effective_context_window_percent": 95,
        "experimental_supported_tools": [],
        "input_modalities": (if kiro { json!(["text"]) } else { json!(["text", "image"]) }),
        "supports_search_tool": !kiro
    })
}

pub(super) fn external_catalog_models(
    codex_home: &Path,
    provider: ExternalCatalogProvider,
    launch_model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<Vec<Value>> {
    let dynamic_models = external_dynamic_catalog_models(codex_home, provider)?;
    let mut models = Vec::with_capacity(provider.models().len() + dynamic_models.len() + 1);
    let launch_model_context_window = dynamic_models
        .iter()
        .find(|model| model.slug.eq_ignore_ascii_case(launch_model))
        .and_then(|model| model.context_window)
        .or_else(|| provider.model_prompt_token_limit(launch_model));
    let default_compact_limit = auto_compact_token_limit;
    let candidates = std::iter::once((launch_model, launch_model_context_window))
        .chain(dynamic_models.iter().map(|model| {
            (
                model.slug.as_str(),
                model
                    .context_window
                    .or_else(|| provider.model_prompt_token_limit(&model.slug)),
            )
        }))
        .chain(
            provider
                .models()
                .iter()
                .map(|model| (model.0, provider.model_prompt_token_limit(model.0))),
        )
        .collect::<Vec<_>>();
    let candidate_ids = candidates.iter().map(|(slug, _)| *slug).collect::<Vec<_>>();
    for index in external_catalog_model_indices(&candidate_ids)? {
        let (slug, per_model_context_window) = candidates[index];
        let slug = slug.trim();
        if models.len() >= prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT {
            bail!(
                "provider model catalog exceeds the hard limit of {} entries",
                prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT
            );
        }
        let priority = models.len() + 1;
        let dynamic_model = dynamic_models
            .iter()
            .find(|model| model.slug.eq_ignore_ascii_case(slug));
        let (fallback_display_name, fallback_description) = provider.model_metadata(slug);
        let display_name = dynamic_model
            .and_then(|model| model.display_name.as_deref())
            .unwrap_or(fallback_display_name);
        let description = dynamic_model
            .and_then(|model| model.description.as_deref())
            .unwrap_or(fallback_description);
        let model_context_window = per_model_context_window.unwrap_or(context_window);
        let model_compact_limit = per_model_context_window
            .map(|cw| cw.saturating_mul(95).saturating_div(100))
            .unwrap_or(default_compact_limit)
            .min(model_context_window.saturating_sub(1));
        models.push(external_catalog_model(
            provider,
            slug,
            display_name,
            description,
            priority,
            model_context_window,
            model_compact_limit,
        ));
    }
    Ok(models)
}

#[cfg(feature = "mojo-core")]
pub(super) fn external_catalog_model_indices(ids: &[&str]) -> Result<Vec<usize>> {
    prodex_mojo_core::rich::merge_catalog_ids(&[], ids)
        .map_err(|error| anyhow::anyhow!("external model catalog merge failed: {error:?}"))
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn external_catalog_model_indices(ids: &[&str]) -> Result<Vec<usize>> {
    Ok(external_catalog_model_indices_rust(ids))
}

#[cfg(any(not(feature = "mojo-core"), test))]
pub(super) fn external_catalog_model_indices_rust(ids: &[&str]) -> Vec<usize> {
    let mut seen = BTreeSet::new();
    ids.iter()
        .enumerate()
        .filter_map(|(index, id)| {
            let id = id.trim();
            (!id.is_empty() && seen.insert(id.to_ascii_lowercase())).then_some(index)
        })
        .collect()
}

#[derive(Clone, Debug)]
struct ExternalDynamicCatalogModel {
    slug: String,
    display_name: Option<String>,
    description: Option<String>,
    context_window: Option<u64>,
}

fn external_dynamic_catalog_models(
    codex_home: &Path,
    provider: ExternalCatalogProvider,
) -> Result<Vec<ExternalDynamicCatalogModel>> {
    let catalog_file = match provider {
        ExternalCatalogProvider::Copilot => COPILOT_RUNTIME_MODEL_CATALOG_FILE,
        ExternalCatalogProvider::Kiro => KIRO_MODEL_CATALOG_FILE,
        ExternalCatalogProvider::Anthropic => return Ok(Vec::new()),
    };
    let catalog_path = codex_home.join(catalog_file);
    let Some(contents) = read_provider_model_catalog_text(&catalog_path)? else {
        return Ok(Vec::new());
    };
    let value = read_external_model_catalog(provider, &contents)?;
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .context("provider model catalog is missing models array")?;
    if models.len() > prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT {
        bail!(
            "provider model catalog exceeds the hard limit of {} entries",
            prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT
        );
    }
    Ok(models
        .iter()
        .filter_map(|model| {
            let slug = model
                .get("id")
                .or_else(|| model.get("slug"))
                .or_else(|| model.get("model"))
                .and_then(Value::as_str)?
                .trim();
            let context_window = copilot_catalog_entry_prompt_token_limit(model)
                .or_else(|| model.get("context_window_tokens").and_then(Value::as_u64))
                .or_else(|| model.get("context_window").and_then(Value::as_u64))
                .filter(|cw| *cw > 1);
            if slug.is_empty() {
                return None;
            }
            Some(ExternalDynamicCatalogModel {
                slug: slug.to_string(),
                display_name: model
                    .get("name")
                    .or_else(|| model.get("model_name"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                description: model
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                context_window,
            })
        })
        .collect())
}

fn copilot_catalog_entry_prompt_token_limit(model: &Value) -> Option<u64> {
    model
        .get("max_prompt_tokens")
        .or_else(|| {
            model
                .get("capabilities")
                .and_then(|c| c.get("limits"))
                .and_then(|l| l.get("max_prompt_tokens"))
        })
        .and_then(Value::as_u64)
        .filter(|tokens| *tokens > 1)
}
