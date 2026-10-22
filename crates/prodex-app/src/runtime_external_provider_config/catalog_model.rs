use super::{
    COPILOT_RUNTIME_MODEL_CATALOG_FILE, ExternalCatalogProvider, read_provider_model_catalog_text,
};
use crate::profile_commands::KIRO_MODEL_CATALOG_FILE;
use anyhow::{Context, Result};
use prodex_provider_core::{
    RuntimeDynamicCatalogModel as ExternalDynamicCatalogModel, RuntimeModelCatalogInput,
    RuntimeModelCatalogPlanInput, normalize_runtime_dynamic_catalog, plan_runtime_model_catalog,
    provider_catalog_entry, render_runtime_model_catalog,
};
use serde_json::{Value, json};
use std::path::Path;

#[cfg(test)]
#[path = "catalog_model/schema_tests.rs"]
mod schema_tests;

#[cfg(test)]
#[path = "catalog_model/dynamic_tests.rs"]
mod dynamic_tests;

#[cfg(test)]
#[path = "catalog_model/assembly_tests.rs"]
mod assembly_tests;

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
    let plan = plan_runtime_model_catalog(RuntimeModelCatalogPlanInput {
        provider: provider.provider_id(),
        launch_model,
        context_window,
        auto_compact_token_limit,
        dynamic_models: &dynamic_models,
    })?;
    plan.into_iter()
        .map(|model| {
            external_catalog_model(
                provider,
                &model.slug,
                &model.display_name,
                &model.description,
                model.priority,
                model.context_window,
                model.auto_compact_token_limit,
            )
        })
        .collect()
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
