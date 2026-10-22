use super::RuntimeModelCatalogError;
use crate::mojo_json::Document;
use serde::Deserialize;
use serde_json::Value;

/// Main-picker metadata selected by the compiled catalog projection, in source order.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct MainCatalogModel {
    pub id: String,
    pub label: String,
    pub priority: u64,
    pub supported: bool,
    pub hidden: bool,
    pub listed: bool,
    pub efforts: Vec<String>,
    pub aliases: Vec<String>,
    pub default_effort: Option<String>,
}

/// Project main-picker metadata; Mojo owns aliases, trimming and defaults.
/// Entries remain in source order, including duplicates and unusable entries.
pub fn project_main_catalog_models(
    entries: &[Value],
) -> Result<Vec<MainCatalogModel>, RuntimeModelCatalogError> {
    let mut document = Document::default();
    document.array(entries);
    let raw = std::str::from_utf8(&document.raw)
        .map_err(|_| RuntimeModelCatalogError::Mojo(prodex_mojo_core::MojoError::InvalidInput))?;
    let output = prodex_mojo_core::json::main_catalog_projection_json(&document.nodes, raw)
        .map_err(RuntimeModelCatalogError::Mojo)?;
    serde_json::from_slice(&output).map_err(RuntimeModelCatalogError::Json)
}

#[cfg(test)]
#[path = "main_projection_tests.rs"]
mod tests;
