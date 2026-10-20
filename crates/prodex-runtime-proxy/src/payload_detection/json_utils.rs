//! Host-only UTF-8 and bounded, borrowed Serde JSON shape acquisition.
//! Compiled Mojo owns the meaningful traversal, field and error decisions.

use prodex_mojo_core::json::{JsonKind, JsonNode};

const RUNTIME_PROXY_JSON_SCAN_LIMIT: usize = 2_048;

pub(super) fn runtime_proxy_utf8_text(body: &[u8]) -> Option<&str> {
    std::str::from_utf8(body).ok().map(str::trim)
}

/// Acquire the first bounded portion of the original Serde tree in generic
/// pre-order. No JSON text is cloned or serialized, and no error classification
/// or key-specific traversal is implemented by the host. Mojo chooses the
/// match by following the linked tree with its own 2048-node search bound.
pub(super) fn runtime_json_borrowed_shape<'a>(
    root: &'a serde_json::Value,
) -> (Vec<JsonNode<'a>>, Vec<&'a serde_json::Value>) {
    let mut pending = vec![(root, None::<usize>, "")];
    let mut nodes: Vec<JsonNode<'a>> = Vec::new();
    let mut values = Vec::new();
    let mut last_child: Vec<Option<usize>> = Vec::new();
    while nodes.len() < RUNTIME_PROXY_JSON_SCAN_LIMIT {
        let Some((value, parent, key)) = pending.pop() else {
            break;
        };
        let index = nodes.len();
        let kind = match value {
            serde_json::Value::Null => JsonKind::Null,
            serde_json::Value::Bool(false) => JsonKind::False,
            serde_json::Value::Bool(true) => JsonKind::True,
            serde_json::Value::Number(_) => JsonKind::Number,
            serde_json::Value::String(_) => JsonKind::String,
            serde_json::Value::Array(_) => JsonKind::Array,
            serde_json::Value::Object(_) => JsonKind::Object,
        };
        nodes.push(JsonNode {
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
            if let Some(previous) = last_child[parent] {
                nodes[previous].next_sibling = Some(index);
            } else {
                nodes[parent].first_child = Some(index);
            }
            last_child[parent] = Some(index);
        }
        match value {
            serde_json::Value::Array(children) => {
                for child in children.iter().rev() {
                    pending.push((child, Some(index), ""));
                }
            }
            serde_json::Value::Object(children) => {
                for (key, child) in children.iter().rev() {
                    pending.push((child, Some(index), key.as_str()));
                }
            }
            _ => {}
        }
    }
    (nodes, values)
}
