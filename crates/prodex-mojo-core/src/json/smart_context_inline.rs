//! Typed boundary for the complete inline-reference projection kernel.
use super::{JsonNode, NodeFfi, StringView, ffi_nodes, signed, status};
use crate::MojoError;

#[derive(Clone, Copy, Debug)]
#[repr(i64)]
pub enum SmartContextInlineOperation {
    Probe,
    Rewrite,
    Expand,
}

pub struct SmartContextInlineDocument<'a> {
    pub nodes: &'a [JsonNode<'a>],
    pub raw: &'a str,
    pub hashes: &'a [&'a str],
}

#[derive(Debug, PartialEq, Eq)]
pub struct SmartContextInlinePlan {
    pub body: Option<Vec<u8>>,
    pub replacements: usize,
    pub candidate_limit_exceeded: bool,
    pub valid: bool,
}

#[derive(Default)]
#[repr(C)]
struct InlineTreeFfi {
    nodes: u64,
    count: i64,
    raw: StringView,
    hashes: u64,
    hash_count: i64,
}

#[repr(C)]
struct InlineInputFfi {
    source: InlineTreeFfi,
    candidate: InlineTreeFfi,
    operation: i64,
    scratch: u64,
    scratch_words: i64,
}

const _: () = {
    assert!(std::mem::size_of::<InlineTreeFfi>() == 48);
    assert!(std::mem::size_of::<InlineInputFfi>() == 120);
};

unsafe extern "C" {
    fn prodex_smart_context_inline_v1(
        abi: i64,
        input: u64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
        metadata_count: i64,
    ) -> i64;
}

impl InlineTreeFfi {
    fn new(nodes: &[NodeFfi], raw: &str, hashes: &[StringView]) -> Result<Self, MojoError> {
        Ok(Self {
            nodes: nodes.as_ptr() as usize as u64,
            count: signed(nodes.len())?,
            raw: raw.into(),
            hashes: hashes.as_ptr() as usize as u64,
            hash_count: signed(hashes.len())?,
        })
    }
}

pub fn smart_context_inline_plan(
    source: SmartContextInlineDocument<'_>,
    candidate: Option<SmartContextInlineDocument<'_>>,
    operation: SmartContextInlineOperation,
) -> Result<SmartContextInlinePlan, MojoError> {
    let nodes = ffi_nodes(source.nodes, source.raw)?;
    let hashes: Vec<StringView> = source.hashes.iter().map(|text| (*text).into()).collect();
    let candidate_nodes = candidate
        .as_ref()
        .map(|document| ffi_nodes(document.nodes, document.raw))
        .transpose()?;
    // ABI v1 stores replacement indices, 56-byte candidates and hash-table slots.
    let scratch_words = nodes
        .len()
        .max(candidate_nodes.as_ref().map_or(0, Vec::len))
        .checked_mul(12)
        .and_then(|count| count.checked_add(1))
        .ok_or(MojoError::Capacity)?;
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(scratch_words)
        .map_err(|_| MojoError::Capacity)?;
    scratch.resize(scratch_words, 0_i64);
    let input = InlineInputFfi {
        source: InlineTreeFfi::new(&nodes, source.raw, &hashes)?,
        candidate: match (candidate_nodes.as_ref(), candidate.as_ref()) {
            (Some(nodes), Some(document)) => InlineTreeFfi::new(nodes, document.raw, &[])?,
            (None, None) => InlineTreeFfi::default(),
            (Some(_), None) | (None, Some(_)) => return Err(MojoError::InvalidInput),
        },
        operation: operation as i64,
        scratch: scratch.as_mut_ptr() as usize as u64,
        scratch_words: signed(scratch.len())?,
    };
    let call = |measuring: bool, output: &mut [u8], metadata: &mut [i64; 3]| {
        status(unsafe {
            prodex_smart_context_inline_v1(
                1,
                (&input as *const InlineInputFfi) as usize as u64,
                i64::from(measuring),
                output.as_mut_ptr() as usize as u64,
                signed(output.len())?,
                metadata.as_mut_ptr() as usize as u64,
                3,
            )
        })
    };
    let mut metadata = [-9_i64; 3];
    call(true, &mut [], &mut metadata)?;
    let (written, valid) = validate_inline_metadata(metadata, operation)?;
    let body = if written == 0 {
        None
    } else {
        let mut output = Vec::new();
        output
            .try_reserve_exact(written)
            .map_err(|_| MojoError::Capacity)?;
        output.resize(written, 0);
        let expected = metadata;
        call(false, &mut output, &mut metadata)?;
        if metadata != expected || std::str::from_utf8(&output).is_err() {
            return Err(MojoError::InvalidOutput);
        }
        Some(output)
    };
    Ok(SmartContextInlinePlan {
        body,
        replacements: if valid {
            usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)?
        } else {
            0
        },
        candidate_limit_exceeded: metadata[2] == 1,
        valid,
    })
}

fn validate_inline_metadata(
    metadata: [i64; 3],
    operation: SmartContextInlineOperation,
) -> Result<(usize, bool), MojoError> {
    let written = usize::try_from(metadata[0]).map_err(|_| MojoError::InvalidOutput)?;
    let valid = metadata[1] >= 0;
    if !matches!(metadata[2], 0 | 1)
        || metadata[1] < -1
        || (!valid && (written != 0 || !matches!(operation, SmartContextInlineOperation::Expand)))
    {
        return Err(MojoError::InvalidOutput);
    }
    let shape_valid = match operation {
        SmartContextInlineOperation::Probe => valid && written == 0,
        SmartContextInlineOperation::Rewrite => valid && (written > 0) == (metadata[1] > 0),
        SmartContextInlineOperation::Expand => metadata[2] == 0 && valid == (written > 0),
    };
    if !shape_valid {
        return Err(MojoError::InvalidOutput);
    }
    Ok((written, valid))
}

#[cfg(test)]
#[path = "smart_context_inline_tests.rs"]
mod tests;
