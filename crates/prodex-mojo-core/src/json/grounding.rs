use super::{JsonNode, MojoError, ffi_nodes, prodex_mojo_gemini_grounding_v1, signed, status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum GeminiGroundingOperation {
    CitationText = 0,
    WebSearchCall = 1,
}

pub fn gemini_grounding(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: GeminiGroundingOperation,
    response_id: Option<&str>,
) -> Result<Vec<u8>, MojoError> {
    let input = ffi_nodes(nodes, raw)?;
    let response_id = response_id.unwrap_or_default();
    let capacity = raw
        .len()
        .checked_mul(6)
        .and_then(|value| value.checked_add(response_id.len().saturating_mul(2)))
        .and_then(|value| value.checked_add(512))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity.max(512)];
    let mut written = 0_i64;
    status(unsafe {
        prodex_mojo_gemini_grounding_v1(
            1,
            operation as i64,
            input.as_ptr() as u64,
            signed(input.len())?,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            response_id.as_ptr() as u64,
            signed(response_id.len())?,
            output.as_mut_ptr() as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok(output)
}
