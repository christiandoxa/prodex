use anyhow::{Context, Result, bail};
use prodex_provider_core::ProviderId;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub(crate) fn parse_kiro_model_catalog_text(text: &str) -> Result<Vec<Value>> {
    let value: Value =
        serde_json::from_str(text).context("failed to parse Kiro model catalog JSON")?;
    normalize_kiro_model_catalog_value(&value)
}

pub(crate) fn normalize_kiro_model_catalog_value(value: &Value) -> Result<Vec<Value>> {
    let models = first_models_array(value).context("Kiro model catalog is missing models array")?;
    normalize_kiro_model_catalog_models(models)
}

pub(crate) fn normalize_kiro_model_catalog_models(models: &[Value]) -> Result<Vec<Value>> {
    if models.len() > prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT {
        bail!(
            "Kiro model catalog exceeds the hard limit of {} entries",
            prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT
        );
    }
    let mut seen = BTreeSet::new();
    let models = models
        .iter()
        .filter_map(|model| {
            let id = first_nonempty_string(model, &["id", "model_id", "modelId", "slug", "model"])?;
            let name =
                first_nonempty_string(model, &["name", "model_name", "modelName"]).unwrap_or(id);
            if !seen.insert(id.to_ascii_lowercase()) {
                return None;
            }
            let mut normalized = serde_json::json!({
                "id": id,
                "name": name,
                "object": "model",
                "owned_by": "kiro-cli",
            });
            if let Some(description) = model.get("description").and_then(Value::as_str) {
                normalized["description"] = Value::String(description.to_string());
            }
            if let Some(context_window) =
                first_positive_u64(model, &["context_window_tokens", "contextWindowTokens"])
            {
                normalized["context_window_tokens"] = Value::from(context_window);
            }
            Some(normalized)
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        bail!("Kiro model catalog returned no usable models");
    }
    prodex_provider_core::merge_provider_model_catalog_json(ProviderId::Kiro, &models)
        .map_err(anyhow::Error::new)?;
    Ok(models)
}

fn first_models_array(value: &Value) -> Option<&[Value]> {
    let object = value.as_object()?;
    first_nonempty_array(
        object,
        &[
            "models",
            "availableModels",
            "available_models",
            "supportedModels",
            "supported_models",
        ],
    )
    .or_else(|| {
        object
            .get("models")
            .and_then(Value::as_object)
            .and_then(|models| {
                first_nonempty_array(
                    models,
                    &[
                        "availableModels",
                        "available_models",
                        "supportedModels",
                        "supported_models",
                    ],
                )
            })
    })
}

fn first_nonempty_array<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a [Value]> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(Value::as_array)
            .filter(|models| !models.is_empty())
            .map(Vec::as_slice)
    })
}

fn first_nonempty_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn first_positive_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
    })
}
