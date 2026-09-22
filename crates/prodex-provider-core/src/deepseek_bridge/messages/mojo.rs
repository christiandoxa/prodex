//! Serde-only message acquisition and application of complete Mojo results.
use crate::mojo_json::Document;
use prodex_mojo_core::deepseek_messages::{
    DeepSeekMessageOperation as Op, transform_deepseek_messages,
};
use serde_json::Value;

fn transform(document: Document<'_>, operation: Op) -> Value {
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    let bytes = transform_deepseek_messages(&document.nodes, raw, operation)
        .expect("Mojo DeepSeek message transform returned invalid output")
        .expect("Mojo DeepSeek message transform must return a result");
    serde_json::from_slice(&bytes).expect("Mojo DeepSeek messages must be valid JSON")
}

fn messages(document: Document<'_>, operation: Op) -> Vec<Value> {
    match transform(document, operation) {
        Value::Array(messages) => messages,
        _ => panic!("Mojo DeepSeek message array must remain an array"),
    }
}

pub(super) fn normalize_thinking(values: &mut [Value]) {
    let mut document = Document::default();
    document.array(values.iter());
    let output = messages(document, Op::ThinkingMessages);
    assert_eq!(
        output.len(),
        values.len(),
        "Mojo thinking normalization preserves message count"
    );
    for (slot, value) in values.iter_mut().zip(output) {
        *slot = value;
    }
}

pub(super) fn normalize_content(value: Value) -> Value {
    let mut document = Document::default();
    document.push(&value, None, "");
    transform(document, Op::AssistantContent)
}

pub(super) fn repair_adjacency(values: &mut Vec<Value>) {
    let mut document = Document::default();
    document.array(values.iter());
    *values = messages(document, Op::RepairAdjacency);
}

pub(super) fn merge_metadata(response: &mut Value, metadata: Option<Value>) {
    let mut document = Document::default();
    let metadata = metadata.unwrap_or(Value::Null);
    document.array([&*response, &metadata]);
    *response = transform(document, Op::MergeMetadata);
}
