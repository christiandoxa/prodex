use crate::{ProviderId, ProviderReasoningEffort, mojo_json::Document};
use prodex_mojo_core::MojoError;
use serde_json::{Value, json};
use std::fmt;

/// Host-observed fields for a Copilot, Kiro or Anthropic Codex model entry.
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

/// A compiled renderer failure or invalid JSON returned across its boundary.
#[derive(Debug)]
pub enum RuntimeModelCatalogError {
    Mojo(MojoError),
    Json(serde_json::Error),
}

impl fmt::Display for RuntimeModelCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mojo(error) => write!(
                formatter,
                "Mojo runtime model catalog renderer failed: {error:?}"
            ),
            Self::Json(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for RuntimeModelCatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mojo(_) => None,
            Self::Json(error) => Some(error),
        }
    }
}

/// Render one external Codex model entry; Mojo owns its schema and defaults.
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
    let mut document = Document::default();
    document.root(&value);
    let raw = std::str::from_utf8(&document.raw)
        .map_err(|_| RuntimeModelCatalogError::Mojo(MojoError::InvalidInput))?;
    let output = prodex_mojo_core::json::runtime_model_catalog_json(&document.nodes, raw)
        .map_err(RuntimeModelCatalogError::Mojo)?;
    serde_json::from_slice(&output).map_err(RuntimeModelCatalogError::Json)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
