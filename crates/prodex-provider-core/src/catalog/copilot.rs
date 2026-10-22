use super::{
    PROVIDER_MODEL_CATALOG_HARD_LIMIT, ProviderModelCatalogLimitError,
    merge_provider_model_catalog_json,
};
use crate::{ProviderId, mojo_json::Document};
use prodex_mojo_core::{MojoError, rich::merge_catalog_ids};
use serde_json::Value;
use std::fmt;

/// Host acquisition/decoding errors or a rejection from the compiled catalog owner.
#[derive(Debug)]
pub enum CopilotModelCatalogError {
    Mojo(MojoError),
    Json(serde_json::Error),
    TooManyModels(String),
    ProviderLimit(ProviderModelCatalogLimitError),
}

impl fmt::Display for CopilotModelCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mojo(error) => write!(
                formatter,
                "Mojo Copilot model catalog transform failed: {error:?}"
            ),
            Self::Json(error) => fmt::Display::fmt(error, formatter),
            Self::TooManyModels(message) => formatter.write_str(message),
            Self::ProviderLimit(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for CopilotModelCatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::ProviderLimit(error) => Some(error),
            Self::Mojo(_) | Self::TooManyModels(_) => None,
        }
    }
}

/// Normalize a parsed Copilot token/model-list payload through the compiled owner.
/// Serde retains JSON acquisition/decoding; existing Mojo owns ID deduplication.
pub fn normalize_copilot_model_catalog(
    value: &Value,
) -> Result<Vec<Value>, CopilotModelCatalogError> {
    let mut document = Document::default();
    document.root(value);
    let raw = std::str::from_utf8(&document.raw)
        .map_err(|_| CopilotModelCatalogError::Mojo(MojoError::InvalidInput))?;
    let output = prodex_mojo_core::json::copilot_model_catalog_json(
        &document.nodes,
        raw,
        PROVIDER_MODEL_CATALOG_HARD_LIMIT,
    )
    .map_err(CopilotModelCatalogError::Mojo)?;
    let candidates = serde_json::from_slice::<Result<Vec<Value>, String>>(&output)
        .map_err(CopilotModelCatalogError::Json)?
        .map_err(CopilotModelCatalogError::TooManyModels)?;
    let ids = candidates
        .iter()
        .map(|model| {
            model
                .get("id")
                .and_then(Value::as_str)
                .ok_or(CopilotModelCatalogError::Mojo(MojoError::InvalidOutput))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let accepted = merge_catalog_ids(&[], &ids).map_err(CopilotModelCatalogError::Mojo)?;
    let models = accepted
        .into_iter()
        .map(|index| {
            candidates
                .get(index)
                .cloned()
                .ok_or(CopilotModelCatalogError::Mojo(MojoError::InvalidOutput))
        })
        .collect::<Result<Vec<_>, _>>()?;
    merge_provider_model_catalog_json(ProviderId::Copilot, &models)
        .map_err(CopilotModelCatalogError::ProviderLimit)?;
    Ok(models)
}
