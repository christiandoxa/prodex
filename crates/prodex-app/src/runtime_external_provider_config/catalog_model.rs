use super::{
    COPILOT_RUNTIME_MODEL_CATALOG_FILE, ExternalCatalogProvider, read_provider_model_catalog_text,
};
use crate::profile_commands::KIRO_MODEL_CATALOG_FILE;
use anyhow::{Context, Result, bail};
use prodex_mojo_core::rich::{CatalogModel, resolve_catalog_model_exact};
use prodex_provider_core::{
    RuntimeDynamicCatalogModel as ExternalDynamicCatalogModel, RuntimeModelCatalogInput,
    normalize_runtime_dynamic_catalog, provider_catalog_entry, render_runtime_model_catalog,
};
use serde_json::{Value, json};
use std::path::Path;

#[cfg(test)]
#[path = "catalog_model/schema_tests.rs"]
mod schema_tests;

#[cfg(test)]
#[path = "catalog_model/dynamic_tests.rs"]
mod dynamic_tests;

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
) -> Result<Value> {
    let catalog_entry = provider_catalog_entry(provider.provider_id(), slug);
    render_runtime_model_catalog(RuntimeModelCatalogInput {
        provider: provider.provider_id(),
        slug,
        display_name,
        description,
        priority,
        context_window,
        auto_compact_token_limit,
        supported_reasoning_efforts: catalog_entry
            .and_then(|entry| entry.supported_reasoning_efforts.as_deref()),
        default_reasoning_effort: catalog_entry.and_then(|entry| entry.default_reasoning_effort),
    })
    .context("failed to render provider model catalog")
}

pub(super) fn external_catalog_models(
    codex_home: &Path,
    provider: ExternalCatalogProvider,
    launch_model: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<Vec<Value>> {
    let dynamic_models = external_dynamic_catalog_models(codex_home, provider)?;
    let static_models = provider.models();
    let mut models = Vec::with_capacity(static_models.len() + dynamic_models.len() + 1);
    let launch_model_context_window = external_dynamic_catalog_model(&dynamic_models, launch_model)
        .and_then(|model| model.context_window)
        .or_else(|| provider.model_prompt_token_limit(launch_model))
        .or_else(|| provider.static_model_context_window(launch_model));
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
        .chain(static_models.iter().map(|model| {
            (
                model.id.as_str(),
                provider
                    .model_prompt_token_limit(&model.id)
                    .or(model.context_window_tokens),
            )
        }))
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
        let dynamic_model = external_dynamic_catalog_model(&dynamic_models, slug);
        let (fallback_display_name, fallback_description) = provider.model_metadata(slug);
        let display_name = dynamic_model
            .and_then(|model| model.display_name.as_deref())
            .unwrap_or(fallback_display_name.as_str());
        let description = dynamic_model
            .and_then(|model| model.description.as_deref())
            .unwrap_or(fallback_description.as_str());
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
        )?);
    }
    Ok(models)
}

pub(super) fn external_catalog_model_indices(ids: &[&str]) -> Result<Vec<usize>> {
    prodex_mojo_core::rich::merge_catalog_ids(&[], ids)
        .map_err(|error| anyhow::anyhow!("external model catalog merge failed: {error:?}"))
}

fn external_dynamic_catalog_model<'a>(
    models: &'a [ExternalDynamicCatalogModel],
    model: &str,
) -> Option<&'a ExternalDynamicCatalogModel> {
    let catalog = models
        .iter()
        .map(|entry| CatalogModel {
            id: &entry.slug,
            aliases: &[],
        })
        .collect::<Vec<_>>();
    resolve_catalog_model_exact(&catalog, model)
        .expect("Mojo exact external catalog lookup failed")
        .map(|index| &models[index])
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
    Ok(normalize_runtime_dynamic_catalog(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_dynamic_catalog_lookup_uses_exact_mojo_identity() {
        let models = vec![ExternalDynamicCatalogModel {
            slug: "Account/Model-X".to_string(),
            display_name: None,
            description: None,
            context_window: Some(123),
        }];
        assert_eq!(
            external_dynamic_catalog_model(&models, "account/model-x")
                .and_then(|model| model.context_window),
            Some(123)
        );
        assert!(external_dynamic_catalog_model(&models, " account/model-x ").is_none());
        assert!(external_dynamic_catalog_model(&models, "missing").is_none());
    }
}
