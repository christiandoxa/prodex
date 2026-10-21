//! Shared Serde acquisition for provider-owned Mojo semantic transforms.
//! This module owns wire-compatible JSON serialization, not provider policy.
use prodex_mojo_core::json::{JsonKind, JsonNode};
use serde_json::Value;

/// Acquires JSON values using the compiled buffered-SSE framing contract.
pub(crate) fn values_from_json_or_sse(body: &[u8]) -> Vec<Value> {
    if let Ok(value) = serde_json::from_slice(body) {
        return vec![value];
    }
    let Ok(text) = std::str::from_utf8(body) else {
        return Vec::new();
    };
    prodex_mojo_core::rich::provider_sse_data_lines(text)
        .expect("Mojo buffered provider SSE framing failed")
        .into_iter()
        .filter_map(|payload| serde_json::from_str(payload).ok())
        .collect()
}

#[derive(Default)]
pub(crate) struct Document<'a> {
    pub(crate) nodes: Vec<JsonNode<'a>>,
    pub(crate) raw: Vec<u8>,
}

impl<'a> Document<'a> {
    /// Borrow a flat Serde shape without serializing large media strings.
    /// This is structure acquisition only: Mojo decides traversal, field
    /// eligibility, and visit order using the stable indices and keys.
    pub(crate) fn borrowed_shape(&mut self, root: &'a Value) -> Vec<&'a Value> {
        assert!(self.nodes.is_empty() && self.raw.is_empty());
        self.append_borrowed_nodes(vec![(root, None, "")], Vec::new())
    }

    /// Acquire many Serde roots as a synthetic array without cloning or
    /// serializing any string payload. Generic shape acquisition is host work.
    pub(crate) fn borrowed_array(&mut self, roots: &'a [Value]) {
        assert!(self.nodes.is_empty() && self.raw.is_empty());
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
        let pending = roots
            .iter()
            .rev()
            .map(|value| (value, Some(0_usize), ""))
            .collect();
        let _ = self.append_borrowed_nodes(pending, vec![None]);
    }

    /// Common host-only node linkage for a borrowed root or synthetic array.
    fn append_borrowed_nodes(
        &mut self,
        mut pending: Vec<(&'a Value, Option<usize>, &'a str)>,
        mut last_child: Vec<Option<usize>>,
    ) -> Vec<&'a Value> {
        let mut values = Vec::new();
        while let Some((value, parent, key)) = pending.pop() {
            let index = self.nodes.len();
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
                raw_start: 0,
                raw_length: 0,
            });
            last_child.push(None);
            values.push(value);
            if let Some(parent) = parent {
                self.link(parent, &mut last_child[parent], index);
            }
            match value {
                Value::Array(children) => {
                    for child in children.iter().rev() {
                        pending.push((child, Some(index), ""));
                    }
                }
                Value::Object(children) => {
                    for (key, child) in children.iter().rev() {
                        pending.push((child, Some(index), key.as_str()));
                    }
                }
                _ => {}
            }
        }
        values
    }

    pub(crate) fn root(&mut self, value: &'a Value) {
        assert!(
            self.nodes.is_empty(),
            "JSON document root can only be written once"
        );
        self.push(value, None, "");
    }

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

    pub(crate) fn openai_chat_request_context(
        &mut self,
        request: &'a Value,
        provider: &'a str,
        default_model: &'a str,
        input_model: Option<&'a str>,
    ) {
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
        let mut previous = None;
        for (key, text) in [
            ("provider", Some(provider)),
            ("default_model", Some(default_model)),
            ("input_model", input_model),
        ] {
            let Some(text) = text else {
                continue;
            };
            if previous.is_some() {
                self.raw.push(b',');
            }
            serde_json::to_writer(&mut self.raw, key).expect("in-memory JSON key serialization");
            self.raw.push(b':');
            let child = self.push_string(text, Some(0), key);
            self.link(0, &mut previous, child);
        }
        if previous.is_some() {
            self.raw.push(b',');
        }
        serde_json::to_writer(&mut self.raw, "request").expect("in-memory JSON key serialization");
        self.raw.push(b':');
        let child = self.push(request, Some(0), "request");
        self.link(0, &mut previous, child);
        self.raw.push(b'}');
        self.nodes[0].raw_length = self.raw.len();
    }

    pub(crate) fn openai_chat_context(
        &mut self,
        response: &'a Value,
        fallback_created_at: Option<u64>,
    ) {
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
        let mut previous = None;

        if let Some(fallback_created_at) = fallback_created_at {
            serde_json::to_writer(&mut self.raw, "fallback_created_at")
                .expect("in-memory JSON key serialization");
            self.raw.push(b':');
            let start = self.raw.len();
            serde_json::to_writer(&mut self.raw, &fallback_created_at)
                .expect("in-memory JSON number serialization");
            let fallback = self.nodes.len();
            self.nodes.push(JsonNode {
                kind: JsonKind::Number,
                first_child: None,
                next_sibling: None,
                parent: Some(0),
                key: "fallback_created_at",
                text: "",
                raw_start: start,
                raw_length: self.raw.len() - start,
            });
            self.link(0, &mut previous, fallback);
        }

        if previous.is_some() {
            self.raw.push(b',');
        }
        serde_json::to_writer(&mut self.raw, "response").expect("in-memory JSON key serialization");
        self.raw.push(b':');
        let response_node = self.push(response, Some(0), "response");
        self.link(0, &mut previous, response_node);
        self.raw.push(b'}');
        self.nodes[0].raw_length = self.raw.len();
    }

    fn push_string(&mut self, text: &'a str, parent: Option<usize>, key: &'a str) -> usize {
        let index = self.nodes.len();
        let start = self.raw.len();
        serde_json::to_writer(&mut self.raw, text).expect("in-memory JSON string serialization");
        self.nodes.push(JsonNode {
            kind: JsonKind::String,
            first_child: None,
            next_sibling: None,
            parent,
            key,
            text,
            raw_start: start,
            raw_length: self.raw.len() - start,
        });
        index
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
