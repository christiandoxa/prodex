//! Serde-only tree acquisition and result materialization for Mojo transforms.
//! No provider policy, field precedence, tool naming or filtering lives here.
use crate::mojo_json::Document;
use prodex_mojo_core::json::{ChatToolOperation, transform_chat_tools};
use serde_json::Value;

pub(super) fn transform_bytes(
    value: &Value,
    operation: ChatToolOperation,
    thinking: bool,
) -> Option<Vec<u8>> {
    let mut document = Document::default();
    match operation {
        ChatToolOperation::Tools | ChatToolOperation::WebSearchOptions => {
            document.member(value, "tools")
        }
        ChatToolOperation::Choice => document.member(value, "tool_choice"),
        ChatToolOperation::WithoutWebSearch | ChatToolOperation::FlattenName => {
            document.push(value, None, "");
        }
    }
    let raw = std::str::from_utf8(&document.raw).expect("Serde emits UTF-8 JSON");
    transform_chat_tools(&document.nodes, raw, operation, thinking)
        .expect("Mojo provider tool transform returned invalid output")
}

pub(super) fn transform_value(
    value: &Value,
    operation: ChatToolOperation,
    thinking: bool,
) -> Option<Value> {
    transform_bytes(value, operation, thinking).map(|bytes| {
        serde_json::from_slice(&bytes).expect("Mojo provider tool transform must return valid JSON")
    })
}
