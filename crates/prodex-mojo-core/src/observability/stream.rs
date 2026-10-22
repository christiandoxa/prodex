use super::{OBSERVABILITY_LABEL_ABI_VERSION, operational_event_source_label};
use crate::MojoError;

// Observability can compile independently of the rich/JSON bridge features.
#[repr(C)]
#[derive(Clone, Copy)]
struct TextView {
    address: u64,
    length: u64,
}

impl From<&str> for TextView {
    fn from(value: &str) -> Self {
        Self {
            address: value.as_ptr() as u64,
            length: value.len() as u64,
        }
    }
}

/// A validated field index and its acquisition-time privacy presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalLoadField {
    Plain(usize),
    Endpoint(usize),
}

/// Human timeline source and optional routine-load field projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationalStreamPlan {
    pub source: &'static str,
    pub load_fields: Option<Vec<OperationalLoadField>>,
}

unsafe extern "C" {
    fn prodex_mojo_operational_log_stream_v1(
        abi_version: i64,
        event_address: u64,
        event_length: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
        output_capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

/// Plan visibility, final source and retained fields without Rust recomputation.
/// Duplicate input keys use the first value; the production map is unique-keyed.
pub fn operational_stream_plan(
    event: &str,
    fields: &[(&str, &str)],
) -> Result<Option<OperationalStreamPlan>, MojoError> {
    if fields.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let views: Vec<TextView> = fields
        .iter()
        .flat_map(|(key, value)| [(*key).into(), (*value).into()])
        .collect();
    let mut output = vec![-1_i64; fields.len()];
    let mut metadata = [-1_i64; 2];
    let status = unsafe {
        prodex_mojo_operational_log_stream_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            event.as_ptr() as u64,
            i64::try_from(event.len()).map_err(|_| MojoError::InvalidInput)?,
            views.as_ptr() as u64,
            fields.len() as i64,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            metadata.as_mut_ptr() as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let load = match metadata[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut selected = Vec::new();
    for (index, tag) in output.into_iter().enumerate() {
        match tag {
            0 => {}
            1 if load => selected.push(OperationalLoadField::Plain(index)),
            2 if load => selected.push(OperationalLoadField::Endpoint(index)),
            _ => return Err(MojoError::InvalidOutput),
        }
    }
    let Some(source) = operational_event_source_label(metadata[0])? else {
        return if load {
            Err(MojoError::InvalidOutput)
        } else {
            Ok(None)
        };
    };
    Ok(Some(OperationalStreamPlan {
        source,
        load_fields: load.then_some(selected),
    }))
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
