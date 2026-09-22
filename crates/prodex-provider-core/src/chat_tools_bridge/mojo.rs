//! Serde-only tree acquisition and result materialization for Mojo transforms.
//! No provider policy, field precedence, tool naming or filtering lives here.
use prodex_mojo_core::json::{ChatToolOperation, JsonKind, JsonNode, transform_chat_tools};
use serde_json::Value;

#[derive(Default)]
struct Document<'a> {
    nodes: Vec<JsonNode<'a>>,
    raw: Vec<u8>,
}

impl<'a> Document<'a> {
    fn member(&mut self, value: &'a Value, key: &'a str) {
        self.nodes.push(JsonNode {
            kind: JsonKind::Object,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 0,
        });
        self.raw.push(b'{');
        if let Some(value) = value.get(key) {
            serde_json::to_writer(&mut self.raw, key).expect("in-memory JSON key serialization");
            self.raw.push(b':');
            let child = self.push(value, Some(0), key);
            self.nodes[0].first_child = Some(child);
        }
        self.raw.push(b'}');
        self.nodes[0].raw_length = self.raw.len();
    }

    fn push(&mut self, value: &'a Value, parent: Option<usize>, key: &'a str) -> usize {
        let index = self.nodes.len();
        let start = self.raw.len();
        let kind = match value {
            Value::Null => JsonKind::Null,
            Value::Bool(false) => JsonKind::False,
            Value::Bool(true) => JsonKind::True,
            Value::Number(_) => JsonKind::Number,
            Value::String(_) => JsonKind::String,
            Value::Array(_) => JsonKind::Array,
            Value::Object(_) => JsonKind::Object,
        };
        self.nodes.push(JsonNode {
            kind,
            first_child: None,
            next_sibling: None,
            parent,
            key,
            text: value.as_str().unwrap_or_default(),
            raw_start: start,
            raw_length: 0,
        });
        let mut previous: Option<usize> = None;
        match value {
            Value::Array(values) => {
                self.raw.push(b'[');
                for value in values {
                    if previous.is_some() {
                        self.raw.push(b',');
                    }
                    let child = self.push(value, Some(index), "");
                    self.link(index, &mut previous, child);
                }
                self.raw.push(b']');
            }
            Value::Object(values) => {
                self.raw.push(b'{');
                for (key, value) in values {
                    if previous.is_some() {
                        self.raw.push(b',');
                    }
                    serde_json::to_writer(&mut self.raw, key)
                        .expect("in-memory JSON key serialization");
                    self.raw.push(b':');
                    let child = self.push(value, Some(index), key);
                    self.link(index, &mut previous, child);
                }
                self.raw.push(b'}');
            }
            _ => serde_json::to_writer(&mut self.raw, value)
                .expect("in-memory JSON scalar serialization"),
        }
        self.nodes[index].raw_length = self.raw.len() - start;
        index
    }

    fn link(&mut self, parent: usize, previous: &mut Option<usize>, child: usize) {
        if let Some(previous) = previous.replace(child) {
            self.nodes[previous].next_sibling = Some(child);
        } else {
            self.nodes[parent].first_child = Some(child);
        }
    }
}

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
