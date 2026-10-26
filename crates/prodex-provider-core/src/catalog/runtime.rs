use crate::{ProviderId, ProviderReasoningEffort, mojo_json::Document};
use prodex_mojo_core::MojoError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fmt;

/// Host-observed fields for a Gemini, DeepSeek, Copilot, Kiro or Anthropic Codex model entry.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeModelCatalogInput<'a> {
    pub provider: ProviderId,
    pub slug: &'a str,
    pub display_name: &'a str,
    pub description: &'a str,
    pub priority: usize,
    pub context_window: u64,
    pub auto_compact_token_limit: u64,
    pub supported_reasoning_efforts: Option<&'a [ProviderReasoningEffort]>,
    pub default_reasoning_effort: Option<ProviderReasoningEffort>,
}

/// A compiled catalog failure, model rejection or invalid returned JSON.
#[derive(Debug)]
pub enum RuntimeModelCatalogError {
    Mojo(MojoError),
    Json(serde_json::Error),
    Rejected(String),
}

impl fmt::Display for RuntimeModelCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mojo(error) => write!(
                formatter,
                "Mojo runtime model catalog renderer failed: {error:?}"
            ),
            Self::Json(error) => fmt::Display::fmt(error, formatter),
            Self::Rejected(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RuntimeModelCatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mojo(_) | Self::Rejected(_) => None,
            Self::Json(error) => Some(error),
        }
    }
}

/// Render one provider Codex model entry; Mojo owns its schema and defaults.
/// Use `render_local_runtime_model_catalog` for Local's fixed metadata.
pub fn render_runtime_model_catalog(
    input: RuntimeModelCatalogInput<'_>,
) -> Result<Value, RuntimeModelCatalogError> {
    let value = json!({
        "provider":input.provider, "slug":input.slug,
        "display_name":input.display_name, "description":input.description,
        "priority":input.priority, "context_window":input.context_window,
        "auto_compact_token_limit":input.auto_compact_token_limit,
        "supported_reasoning_efforts":input.supported_reasoning_efforts,
        "default_reasoning_effort":input.default_reasoning_effort,
    });
    render_runtime_model_catalog_value(&value)
}

/// Render Local's fixed metadata and schema from its observed slug and budgets.
/// Mojo chooses descriptions, priority, reasoning levels and capability fields.
pub fn render_local_runtime_model_catalog(
    slug: &str,
    context_window: u64,
    auto_compact_token_limit: u64,
) -> Result<Value, RuntimeModelCatalogError> {
    render_runtime_model_catalog_value(&json!({
        "provider":ProviderId::Local, "slug":slug,
        "context_window":context_window, "auto_compact_token_limit":auto_compact_token_limit,
    }))
}

fn render_runtime_model_catalog_value(value: &Value) -> Result<Value, RuntimeModelCatalogError> {
    let mut document = Document::default();
    document.root(value);
    let raw = std::str::from_utf8(&document.raw)
        .map_err(|_| RuntimeModelCatalogError::Mojo(MojoError::InvalidInput))?;
    let output = prodex_mojo_core::json::runtime_model_catalog_json(&document.nodes, raw)
        .map_err(RuntimeModelCatalogError::Mojo)?;
    serde_json::from_slice(&output).map_err(RuntimeModelCatalogError::Json)
}

/// Runtime metadata projected by the compiled stored-catalog owner, in input order.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RuntimeDynamicCatalogModel {
    pub slug: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub context_window: Option<u64>,
}

/// Select stored Copilot/Kiro runtime fields without deduplicating entries.
pub fn normalize_runtime_dynamic_catalog(
    value: &Value,
) -> Result<Vec<RuntimeDynamicCatalogModel>, RuntimeModelCatalogError> {
    let mut document = Document::default();
    document.root(value);
    let raw = std::str::from_utf8(&document.raw)
        .map_err(|_| RuntimeModelCatalogError::Mojo(MojoError::InvalidInput))?;
    let output = prodex_mojo_core::json::runtime_dynamic_catalog_json(
        &document.nodes,
        raw,
        super::PROVIDER_MODEL_CATALOG_HARD_LIMIT,
    )
    .map_err(RuntimeModelCatalogError::Mojo)?;
    serde_json::from_slice::<Result<Vec<RuntimeDynamicCatalogModel>, String>>(&output)
        .map_err(RuntimeModelCatalogError::Json)?
        .map_err(RuntimeModelCatalogError::Rejected)
}

/// One compiled model plan passed unchanged to the existing runtime schema renderer.
#[derive(Debug, Deserialize)]
pub struct RuntimePlannedCatalogModel {
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub priority: usize,
    pub context_window: u64,
    pub auto_compact_token_limit: u64,
}

/// Host metadata for the compiled launch/dynamic/static runtime catalog plan.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeModelCatalogPlanInput<'a> {
    pub provider: ProviderId,
    pub launch_model: &'a str,
    pub context_window: u64,
    pub auto_compact_token_limit: u64,
    pub dynamic_models: &'a [RuntimeDynamicCatalogModel],
}

/// Plan Gemini, DeepSeek, Copilot, Kiro or Anthropic metadata, ordering and token budgets.
pub fn plan_runtime_model_catalog(
    input: RuntimeModelCatalogPlanInput<'_>,
) -> Result<Vec<RuntimePlannedCatalogModel>, RuntimeModelCatalogError> {
    // DeepSeek retains its distinct legacy runtime entries and descriptions.
    let static_models = if input.provider == ProviderId::DeepSeek {
        let models = prodex_mojo_core::super_provider_config::deepseek_catalog_static_models()
            .map_err(RuntimeModelCatalogError::Mojo)?;
        json!(
            models
                .into_iter()
                .map(|model| json!({
                    "id":model.slug, "display_name":model.display_name,
                    "description":model.description, "context_window_tokens":null, "aliases":[],
                }))
                .collect::<Vec<_>>()
        )
    } else {
        json!(super::provider_catalog_entries_for(input.provider))
    };
    let value = json!({
        "provider":input.provider, "launch_model":input.launch_model,
        "context_window":input.context_window,
        "auto_compact_token_limit":input.auto_compact_token_limit,
        "dynamic_models":input.dynamic_models,
        "static_models":static_models,
    });
    let mut document = Document::default();
    document.root(&value);
    let raw = std::str::from_utf8(&document.raw)
        .map_err(|_| RuntimeModelCatalogError::Mojo(MojoError::InvalidInput))?;
    let output = prodex_mojo_core::json::runtime_catalog_plan_json(
        &document.nodes,
        raw,
        super::PROVIDER_MODEL_CATALOG_HARD_LIMIT,
    )
    .map_err(RuntimeModelCatalogError::Mojo)?;
    let mut models =
        serde_json::from_slice::<Result<Vec<RuntimePlannedCatalogModel>, String>>(&output)
            .map_err(RuntimeModelCatalogError::Json)?
            .map_err(RuntimeModelCatalogError::Rejected)?;
    models.retain(|model| !super::provider_model_is_retired(input.provider, &model.slug));
    Ok(models)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
