use anyhow::{Context, Result, bail};
use prodex_mojo_core::json::{JsonKind, JsonNode};

pub(super) const THREAD_INDEX_MAX_JSON_BYTES: usize = 64 * 1024 * 1024;
pub(super) const THREAD_INDEX_MAX_JSON_NODES: usize = 1_048_576;

#[derive(serde::Deserialize)]
struct DirtyMarkerBoundary {
    schema_version: serde_json::Value,
    rollout_path: serde_json::Value,
}

pub(super) struct ThreadIndexJsonTree<'a> {
    pub(super) nodes: Vec<JsonNode<'a>>,
    pub(super) raw: Vec<u8>,
}

pub(super) fn thread_index_json_tree(value: &serde_json::Value) -> Result<ThreadIndexJsonTree<'_>> {
    let mut tree = ThreadIndexJsonTree {
        nodes: Vec::new(),
        raw: Vec::new(),
    };
    append_json_node(value, "", None, &mut tree)?;
    if tree.raw.len() > THREAD_INDEX_MAX_JSON_BYTES {
        bail!("Mojo thread-index normalized JSON exceeded its ABI bound");
    }
    Ok(tree)
}

pub(super) fn dirty_marker_value(contents: &[u8]) -> Option<serde_json::Value> {
    let text = std::str::from_utf8(contents).ok()?;
    let boundary = serde_json::from_str::<DirtyMarkerBoundary>(text).ok()?;
    Some(serde_json::Value::Object(serde_json::Map::from_iter([
        ("schema_version".to_string(), boundary.schema_version),
        ("rollout_path".to_string(), boundary.rollout_path),
    ])))
}

fn append_json_node<'a>(
    value: &'a serde_json::Value,
    key: &'a str,
    parent: Option<usize>,
    tree: &mut ThreadIndexJsonTree<'a>,
) -> Result<usize> {
    if tree.nodes.len() >= THREAD_INDEX_MAX_JSON_NODES {
        bail!("Mojo thread-index JSON tree exceeded its ABI bound");
    }
    tree.nodes
        .try_reserve(1)
        .map_err(|_| anyhow::anyhow!("failed to allocate thread-index JSON nodes"))?;
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
    let mut previous = None;
    match value {
        serde_json::Value::Array(values) => {
            tree.raw.push(b'[');
            for value in values {
                if previous.is_some() {
                    tree.raw.push(b',');
                }
                let child = append_json_node(value, "", Some(index), tree)?;
                link_json_node(&mut tree.nodes, index, &mut previous, child);
            }
            tree.raw.push(b']');
        }
        serde_json::Value::Object(values) => {
            tree.raw.push(b'{');
            for (key, value) in values {
                if previous.is_some() {
                    tree.raw.push(b',');
                }
                serde_json::to_writer(&mut tree.raw, key)
                    .context("failed to encode thread-index JSON key")?;
                tree.raw.push(b':');
                let child = append_json_node(value, key, Some(index), tree)?;
                link_json_node(&mut tree.nodes, index, &mut previous, child);
            }
            tree.raw.push(b'}');
        }
        _ => serde_json::to_writer(&mut tree.raw, value)
            .context("failed to encode thread-index JSON value")?,
    }
    tree.nodes[index].raw_length = tree.raw.len() - start;
    Ok(index)
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
