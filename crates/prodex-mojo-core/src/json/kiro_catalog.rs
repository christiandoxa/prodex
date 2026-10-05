use super::{JsonKind, JsonNode, ffi_nodes, signed, status};
use crate::MojoError;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KiroModelCatalogRecordFfi {
    source_index: i64,
    id_node: i64,
    id_start: i64,
    id_length: i64,
    name_node: i64,
    name_start: i64,
    name_length: i64,
    description_node: i64,
    context_window_tokens: u64,
    context_present: i64,
}

const _: () = assert!(std::mem::size_of::<KiroModelCatalogRecordFfi>() == 80);

unsafe extern "C" {
    fn prodex_mojo_kiro_catalog_normalize_v1(
        abi_version: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        max_entries: i64,
        records_address: u64,
        records_capacity: i64,
        result_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KiroModelCatalogModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub context_window_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KiroModelCatalogPlan {
    Ready {
        input_count: usize,
        models: Vec<KiroModelCatalogModel>,
    },
    MissingModelsArray,
    TooManyModels {
        input_count: usize,
    },
    NoUsableModels,
}

fn catalog_string_span(
    nodes: &[JsonNode<'_>],
    node_index: i64,
    start: i64,
    length: i64,
) -> Result<String, MojoError> {
    let node_index = usize::try_from(node_index).map_err(|_| MojoError::InvalidOutput)?;
    let node = nodes.get(node_index).ok_or(MojoError::InvalidOutput)?;
    if !matches!(node.kind, JsonKind::String) {
        return Err(MojoError::InvalidOutput);
    }
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let length = usize::try_from(length).map_err(|_| MojoError::InvalidOutput)?;
    let end = start.checked_add(length).ok_or(MojoError::InvalidOutput)?;
    let value = node.text.get(start..end).ok_or(MojoError::InvalidOutput)?;
    if value.is_empty() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(value.to_string())
}

/// Select and normalize Kiro model entries from a Serde-built JSON tree.
///
/// Mojo owns array precedence, field aliases, Unicode trimming, metadata
/// selection, and the input entry cap. Rust maps validated spans into strings.
pub fn kiro_model_catalog_plan(
    nodes: &[JsonNode<'_>],
    raw: &str,
    max_entries: usize,
) -> Result<KiroModelCatalogPlan, MojoError> {
    if max_entries == 0 {
        return Err(MojoError::InvalidInput);
    }
    let input = ffi_nodes(nodes, raw)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(max_entries)
        .map_err(|_| MojoError::Capacity)?;
    records.resize(max_entries, KiroModelCatalogRecordFfi::default());
    let mut result = [-1_i64; 3];
    status(unsafe {
        prodex_mojo_kiro_catalog_normalize_v1(
            1,
            input.as_ptr() as u64,
            signed(input.len())?,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            signed(max_entries)?,
            records.as_mut_ptr() as u64,
            signed(records.len())?,
            result.as_mut_ptr() as u64,
        )
    })?;

    match result[0] {
        1 => Ok(KiroModelCatalogPlan::MissingModelsArray),
        2 => Ok(KiroModelCatalogPlan::TooManyModels {
            input_count: usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?,
        }),
        3 => Ok(KiroModelCatalogPlan::NoUsableModels),
        0 => {
            let input_count = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
            let records_written =
                usize::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?;
            if input_count > max_entries
                || records_written == 0
                || records_written > input_count
                || records_written > records.len()
            {
                return Err(MojoError::InvalidOutput);
            }
            let mut models = Vec::new();
            models
                .try_reserve_exact(records_written)
                .map_err(|_| MojoError::Capacity)?;
            let mut previous_source_index = None;
            for record in &records[..records_written] {
                let source_index =
                    usize::try_from(record.source_index).map_err(|_| MojoError::InvalidOutput)?;
                if source_index >= input_count
                    || previous_source_index.is_some_and(|previous| source_index <= previous)
                {
                    return Err(MojoError::InvalidOutput);
                }
                previous_source_index = Some(source_index);
                let id =
                    catalog_string_span(nodes, record.id_node, record.id_start, record.id_length)?;
                let name = catalog_string_span(
                    nodes,
                    record.name_node,
                    record.name_start,
                    record.name_length,
                )?;
                let description = if record.description_node == -1 {
                    None
                } else {
                    let index = usize::try_from(record.description_node)
                        .map_err(|_| MojoError::InvalidOutput)?;
                    let node = nodes.get(index).ok_or(MojoError::InvalidOutput)?;
                    if !matches!(node.kind, JsonKind::String) {
                        return Err(MojoError::InvalidOutput);
                    }
                    Some(node.text.to_string())
                };
                let context_window_tokens = match record.context_present {
                    0 if record.context_window_tokens == 0 => None,
                    1 if record.context_window_tokens > 0 => Some(record.context_window_tokens),
                    _ => return Err(MojoError::InvalidOutput),
                };
                models.push(KiroModelCatalogModel {
                    id,
                    name,
                    description,
                    context_window_tokens,
                });
            }
            Ok(KiroModelCatalogPlan::Ready {
                input_count,
                models,
            })
        }
        _ => Err(MojoError::InvalidOutput),
    }
}
