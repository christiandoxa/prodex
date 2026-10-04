use anyhow::{Context, Result};
use serde_json::Value;

pub(crate) fn parse_kiro_model_catalog_text(text: &str) -> Result<Vec<Value>> {
    let value: Value =
        serde_json::from_str(text).context("failed to parse Kiro model catalog JSON")?;
    normalize_kiro_model_catalog_value(&value)
}

pub(crate) fn normalize_kiro_model_catalog_value(value: &Value) -> Result<Vec<Value>> {
    prodex_provider_core::normalize_kiro_model_catalog(value).map_err(anyhow::Error::new)
}

pub(crate) fn normalize_kiro_model_catalog_models(models: &[Value]) -> Result<Vec<Value>> {
    prodex_provider_core::normalize_kiro_model_catalog_models(models).map_err(anyhow::Error::new)
}
