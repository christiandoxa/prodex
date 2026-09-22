//! Shared Serde acquisition for provider-owned Mojo semantic transforms.
//! This module owns wire-compatible JSON serialization, not provider policy.
use prodex_mojo_core::json::{JsonKind, JsonNode};
use serde_json::Value;

#[derive(Default)]
pub(crate) struct Document<'a> {
    pub(crate) nodes: Vec<JsonNode<'a>>,
    pub(crate) raw: Vec<u8>,
}

impl<'a> Document<'a> {
    pub(crate) fn array(&mut self, values: impl IntoIterator<Item = &'a Value>) {
        self.nodes.push(JsonNode {
            kind: JsonKind::Array,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 0,
        });
        self.raw.push(b'[');
        let mut previous = None;
        for value in values {
            if previous.is_some() {
                self.raw.push(b',');
            }
            let child = self.push(value, Some(0), "");
            self.link(0, &mut previous, child);
        }
        self.raw.push(b']');
        self.nodes[0].raw_length = self.raw.len();
    }

    pub(crate) fn member(&mut self, value: &'a Value, key: &'a str) {
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

    pub(crate) fn push(&mut self, value: &'a Value, parent: Option<usize>, key: &'a str) -> usize {
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
