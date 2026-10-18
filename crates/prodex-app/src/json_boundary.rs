//! Bounded Serde acquisition shared by application-owned Mojo projections.
use anyhow::{Context, Result, bail};
use prodex_mojo_core::json::{JsonKind, JsonNode};

pub(crate) const APP_JSON_MAX_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const APP_JSON_MAX_NODES: usize = 1_048_576;

pub(crate) struct AppJsonTree<'a> {
    pub(crate) nodes: Vec<JsonNode<'a>>,
    pub(crate) raw: Vec<u8>,
    acquired_bytes: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JsonWire {
    Encode,
    Omit,
}

pub(crate) fn app_json_tree(value: &serde_json::Value) -> Result<AppJsonTree<'_>> {
    acquire_json_tree(value, JsonWire::Encode)
}

/// Acquires decoded fields for kernels that never read JSON wire spans.
pub(crate) fn app_json_tree_without_wire(value: &serde_json::Value) -> Result<AppJsonTree<'_>> {
    acquire_json_tree(value, JsonWire::Omit)
}

fn acquire_json_tree(value: &serde_json::Value, wire: JsonWire) -> Result<AppJsonTree<'_>> {
    let mut tree = AppJsonTree {
        nodes: Vec::new(),
        raw: Vec::new(),
        acquired_bytes: 0,
    };
    append_json_node(value, "", None, &mut tree, wire)?;
    if tree.raw.len() > APP_JSON_MAX_BYTES {
        bail!("Mojo application normalized JSON exceeded its ABI bound");
    }
    Ok(tree)
}

fn append_json_node<'a>(
    value: &'a serde_json::Value,
    key: &'a str,
    parent: Option<usize>,
    tree: &mut AppJsonTree<'a>,
    wire: JsonWire,
) -> Result<usize> {
    if tree.nodes.len() >= APP_JSON_MAX_NODES {
        bail!("Mojo application JSON tree exceeded its ABI bound");
    }
    tree.acquired_bytes = tree
        .acquired_bytes
        .checked_add(key.len())
        .and_then(|count| count.checked_add(value.as_str().map_or(0, str::len)))
        .filter(|count| *count <= APP_JSON_MAX_BYTES)
        .ok_or_else(|| anyhow::anyhow!("Mojo application decoded JSON exceeded its ABI bound"))?;
    tree.nodes
        .try_reserve(1)
        .map_err(|_| anyhow::anyhow!("failed to allocate application JSON nodes"))?;
    let index = tree.nodes.len();
    let start = tree.raw.len();
    let kind = match value {
        serde_json::Value::Null => JsonKind::Null,
        serde_json::Value::Bool(false) => JsonKind::False,
        serde_json::Value::Bool(true) => JsonKind::True,
        serde_json::Value::Number(_) => JsonKind::Number,
        serde_json::Value::String(_) => JsonKind::String,
        serde_json::Value::Array(_) => JsonKind::Array,
        serde_json::Value::Object(_) => JsonKind::Object,
    };
    tree.nodes.push(JsonNode {
        kind,
        first_child: None,
        next_sibling: None,
        parent,
        key,
        text: value.as_str().unwrap_or_default(),
        raw_start: start,
        raw_length: 0,
    });
    append_json_contents(value, index, tree, wire)?;
    tree.nodes[index].raw_length = tree.raw.len() - start;
    Ok(index)
}

fn append_json_contents<'a>(
    value: &'a serde_json::Value,
    index: usize,
    tree: &mut AppJsonTree<'a>,
    wire: JsonWire,
) -> Result<()> {
    match value {
        serde_json::Value::Array(values) => append_json_array(values, index, tree, wire),
        serde_json::Value::Object(values) => append_json_object(values, index, tree, wire),
        _ if wire == JsonWire::Encode => serde_json::to_writer(&mut tree.raw, value)
            .context("failed to encode application JSON value"),
        _ => Ok(()),
    }
}

fn write_json_token(tree: &mut AppJsonTree<'_>, wire: JsonWire, token: u8) {
    if wire == JsonWire::Encode {
        tree.raw.push(token);
    }
}

fn append_json_array<'a>(
    values: &'a [serde_json::Value],
    index: usize,
    tree: &mut AppJsonTree<'a>,
    wire: JsonWire,
) -> Result<()> {
    write_json_token(tree, wire, b'[');
    let mut previous = None;
    for value in values {
        if previous.is_some() {
            write_json_token(tree, wire, b',');
        }
        let child = append_json_node(value, "", Some(index), tree, wire)?;
        link_json_node(&mut tree.nodes, index, &mut previous, child);
    }
    write_json_token(tree, wire, b']');
    Ok(())
}

fn append_json_object<'a>(
    values: &'a serde_json::Map<String, serde_json::Value>,
    index: usize,
    tree: &mut AppJsonTree<'a>,
    wire: JsonWire,
) -> Result<()> {
    write_json_token(tree, wire, b'{');
    let mut previous = None;
    for (key, value) in values {
        if previous.is_some() {
            write_json_token(tree, wire, b',');
        }
        if wire == JsonWire::Encode {
            serde_json::to_writer(&mut tree.raw, key)
                .context("failed to encode application JSON key")?;
            tree.raw.push(b':');
        }
        let child = append_json_node(value, key, Some(index), tree, wire)?;
        link_json_node(&mut tree.nodes, index, &mut previous, child);
    }
    write_json_token(tree, wire, b'}');
    Ok(())
}

fn link_json_node(
    nodes: &mut [JsonNode<'_>],
    parent: usize,
    previous: &mut Option<usize>,
    child: usize,
) {
    if let Some(previous) = previous.replace(child) {
        nodes[previous].next_sibling = Some(child);
    } else {
        nodes[parent].first_child = Some(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquisition_modes_preserve_topology_and_exact_wire_encoding() {
        let source = serde_json::json!({
            "opaque": [{"text": "東京\nquoted\"", "null": null}, false, 2.5],
            "empty": [], "object": {}, "flag": true,
        });
        let encoded = app_json_tree(&source).unwrap();
        let omitted = app_json_tree_without_wire(&source).unwrap();
        assert_eq!(encoded.raw, serde_json::to_vec(&source).unwrap());
        assert!(omitted.raw.is_empty());
        assert_eq!(encoded.nodes.len(), omitted.nodes.len());
        for (a, b) in encoded.nodes.iter().zip(&omitted.nodes) {
            assert_eq!(
                (a.first_child, a.next_sibling, a.parent, a.key, a.text),
                (b.first_child, b.next_sibling, b.parent, b.key, b.text)
            );
            assert_eq!(b.raw_length, 0);
            assert_eq!(b.raw_start, 0);
        }
    }
}
