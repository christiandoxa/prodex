use super::{PROVIDER_MODEL_CATALOG_HARD_LIMIT, merge_provider_model_catalog_json};
use crate::ProviderId;
use crate::mojo_json::Document;
use prodex_mojo_core::json::{JsonNode, KiroModelCatalogPlan, kiro_model_catalog_plan};
use prodex_mojo_core::rich::merge_catalog_ids;
use serde_json::Value;
use std::fmt;

#[derive(Debug)]
pub enum KiroModelCatalogError {
    Mojo(prodex_mojo_core::MojoError),
    MissingModelsArray,
    TooManyModels { limit: usize },
    NoUsableModels,
    ProviderLimit(super::ProviderModelCatalogLimitError),
}

impl fmt::Display for KiroModelCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mojo(error) => {
                write!(formatter, "Mojo Kiro model catalog plan failed: {error:?}")
            }
            Self::MissingModelsArray => {
                formatter.write_str("Kiro model catalog is missing models array")
            }
            Self::TooManyModels { limit } => write!(
                formatter,
                "Kiro model catalog exceeds the hard limit of {limit} entries"
            ),
            Self::NoUsableModels => {
                formatter.write_str("Kiro model catalog returned no usable models")
            }
            Self::ProviderLimit(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for KiroModelCatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProviderLimit(error) => Some(error),
            _ => None,
        }
    }
}

fn normalize_document(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Vec<Value>, KiroModelCatalogError> {
    let plan = kiro_model_catalog_plan(nodes, raw, PROVIDER_MODEL_CATALOG_HARD_LIMIT)
        .map_err(KiroModelCatalogError::Mojo)?;
    let entries = match plan {
        KiroModelCatalogPlan::Ready { models, .. } => models,
        KiroModelCatalogPlan::MissingModelsArray => {
            return Err(KiroModelCatalogError::MissingModelsArray);
        }
        KiroModelCatalogPlan::TooManyModels { .. } => {
            return Err(KiroModelCatalogError::TooManyModels {
                limit: PROVIDER_MODEL_CATALOG_HARD_LIMIT,
            });
        }
        KiroModelCatalogPlan::NoUsableModels => {
            return Err(KiroModelCatalogError::NoUsableModels);
        }
    };

    let normalized = entries
        .iter()
        .map(|entry| {
            let mut model = serde_json::json!({
                "id": entry.id,
                "name": entry.name,
                "object": "model",
                "owned_by": "kiro-cli",
            });
            if let Some(description) = &entry.description {
                model["description"] = Value::String(description.clone());
            }
            if let Some(context_window_tokens) = entry.context_window_tokens {
                model["context_window_tokens"] = Value::from(context_window_tokens);
            }
            model
        })
        .collect::<Vec<_>>();
    let ids = entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();
    let accepted = merge_catalog_ids(&[], &ids).map_err(KiroModelCatalogError::Mojo)?;
    let models = accepted
        .into_iter()
        .map(|index| {
            normalized
                .get(index)
                .cloned()
                .ok_or(KiroModelCatalogError::Mojo(
                    prodex_mojo_core::MojoError::InvalidOutput,
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if models.is_empty() {
        return Err(KiroModelCatalogError::Mojo(
            prodex_mojo_core::MojoError::InvalidOutput,
        ));
    }
    merge_provider_model_catalog_json(ProviderId::Kiro, &models)
        .map_err(KiroModelCatalogError::ProviderLimit)?;
    Ok(models)
}

/// Normalize one parsed Kiro catalog or model array through the Kiro Mojo plan.
pub fn normalize_kiro_model_catalog(value: &Value) -> Result<Vec<Value>, KiroModelCatalogError> {
    let mut document = Document::default();
    if document.push(value, None, "") != 0 {
        return Err(KiroModelCatalogError::Mojo(
            prodex_mojo_core::MojoError::InvalidInput,
        ));
    }
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    normalize_document(&document.nodes, raw)
}

/// Normalize an already extracted model list through the Kiro Mojo plan.
pub fn normalize_kiro_model_catalog_models(
    models: &[Value],
) -> Result<Vec<Value>, KiroModelCatalogError> {
    let mut document = Document::default();
    document.array(models.iter());
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    normalize_document(&document.nodes, raw)
}

#[cfg(test)]
mod tests {
    use super::{normalize_kiro_model_catalog, normalize_kiro_model_catalog_models};
    use serde_json::json;

    #[test]
    fn kiro_model_catalog_uses_mojo_precedence_and_preserves_model_metadata() {
        let models = normalize_kiro_model_catalog(&json!({
            "models": [
                {
                    "id": " primary ",
                    "model_id": "secondary",
                    "name": " ",
                    "modelName": "Display",
                    "description": "details",
                    "context_window_tokens": 0,
                    "contextWindowTokens": 456
                },
                {"modelId": "PRIMARY", "name": "duplicate"},
                {"slug": "other", "description": ""},
                {"id": "  "},
                false
            ],
            "availableModels": [{"id": "ignored"}]
        }))
        .unwrap();

        assert_eq!(
            models,
            vec![
                json!({
                    "id": "primary",
                    "name": "Display",
                    "object": "model",
                    "owned_by": "kiro-cli",
                    "description": "details",
                    "context_window_tokens": 456
                }),
                json!({
                    "id": "other",
                    "name": "other",
                    "object": "model",
                    "owned_by": "kiro-cli",
                    "description": ""
                })
            ]
        );
    }

    #[test]
    fn kiro_model_catalog_models_entry_point_uses_array_root() {
        let models = normalize_kiro_model_catalog_models(&[json!({
            "model_id": "model-a",
            "model_name": "Model A"
        })])
        .unwrap();

        assert_eq!(models[0]["id"], "model-a");
        assert_eq!(models[0]["name"], "Model A");
    }
}
