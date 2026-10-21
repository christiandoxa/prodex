//! Serde acquisition and host layout facts for the shared Mojo cache estimator.
use crate::mojo_json::Document;
use prodex_mojo_core::{
    MojoError,
    json::{self, JsonMemoryLayout},
};
use serde_json::Value;

/// Estimates the cache-owned key and JSON values without serializing their strings.
pub fn provider_core_conversation_estimated_bytes(
    storage_key: &str,
    messages: &[Value],
) -> Result<usize, MojoError> {
    let mut document = Document::default();
    document.borrowed_array(messages);
    json::conversation_estimated_bytes(
        &document.nodes,
        storage_key.len(),
        JsonMemoryLayout {
            value_bytes: std::mem::size_of::<Value>(),
            bool_bytes: std::mem::size_of::<bool>(),
            number_bytes: std::mem::size_of::<serde_json::Number>(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_memory_estimates_nested_unicode_values_without_synthetic_root_cost() {
        let messages = vec![
            Value::Null,
            Value::Bool(false),
            Value::Bool(true),
            serde_json::json!(-42),
            serde_json::json!("é"),
            serde_json::json!([null, {"鍵": "値", "x": []}]),
        ];
        let expected = "resp-".len()
            + 10 * std::mem::size_of::<Value>()
            + 2 * std::mem::size_of::<bool>()
            + std::mem::size_of::<serde_json::Number>()
            + "é".len()
            + "鍵".len()
            + "値".len()
            + "x".len();
        assert_eq!(
            provider_core_conversation_estimated_bytes("resp-", &messages),
            Ok(expected)
        );
        assert_eq!(provider_core_conversation_estimated_bytes("é", &[]), Ok(2));
    }
}
