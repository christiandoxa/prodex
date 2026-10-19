//! Serde and static-lifetime adapters for compiled Gemini 3 descriptions.

use prodex_mojo_core::gemini_tooling_policy::gemini_model_uses_gemini3_toolset;
use prodex_mojo_core::json::{GeminiToolDescriptionOperation, gemini_tool_description_policy};
use std::sync::OnceLock;

fn description_policy<T: serde::de::DeserializeOwned>(
    operation: GeminiToolDescriptionOperation,
    input: &serde_json::Value,
) -> T {
    let mut document = crate::mojo_json::Document::default();
    document.root(input);
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let output = gemini_tool_description_policy(&document.nodes, raw, operation)
        .expect("compiled Mojo Gemini tool-description policy failed");
    serde_json::from_slice(&output)
        .expect("compiled Mojo Gemini tool-description policy returned an invalid value")
}

pub fn gemini_provider_core_apply_gemini3_tool_declaration_overrides(
    model: &str,
    declarations: &mut [serde_json::Value],
) {
    let output: Vec<serde_json::Value> = description_policy(
        GeminiToolDescriptionOperation::ApplyOverrides,
        &serde_json::json!({"model":model,"declarations":declarations}),
    );
    assert_eq!(
        output.len(),
        declarations.len(),
        "compiled Mojo Gemini description override changed declaration count"
    );
    for (slot, value) in declarations.iter_mut().zip(output) {
        *slot = value;
    }
}

pub fn gemini_provider_core_model_uses_gemini3_toolset(model: &str) -> bool {
    gemini_model_uses_gemini3_toolset(model)
        .expect("Mojo Gemini-3 model policy should accept Rust strings")
}

pub fn gemini_provider_core_gemini3_tool_description(name: &str) -> Option<&'static str> {
    let index: Option<usize> = description_policy(
        GeminiToolDescriptionOperation::DescriptionIndex,
        &serde_json::Value::String(name.to_string()),
    );
    let index = index?;
    static DESCRIPTIONS: OnceLock<Vec<String>> = OnceLock::new();
    let descriptions = DESCRIPTIONS.get_or_init(|| {
        description_policy(
            GeminiToolDescriptionOperation::Catalog,
            &serde_json::Value::Null,
        )
    });
    Some(
        descriptions
            .get(index)
            .expect("compiled Mojo Gemini description index is outside its catalog")
            .as_str(),
    )
}

#[cfg(test)]
#[path = "gemini3_tests.rs"]
mod tests;
