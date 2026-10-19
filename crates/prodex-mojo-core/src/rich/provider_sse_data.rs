//! Borrowed payloads from buffered provider SSE bodies.
use super::MojoError;

unsafe extern "C" {
    fn prodex_sse_buffered_data_spans_v1(
        version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        capacity: i64,
        required_address: u64,
    ) -> i64;
}

/// Returns ordered, Unicode-trimmed `data:` payloads, including empty payloads.
/// Buffered error bodies allow leading whitespace; runtime streaming uses its
/// existing strict SSE contract. Mojo owns recognition and byte boundaries.
pub fn provider_sse_data_lines(text: &str) -> Result<Vec<&str>, MojoError> {
    let length = i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut required = u64::MAX;
    let call = |output: u64, capacity: i64, required: &mut u64| unsafe {
        prodex_sse_buffered_data_spans_v1(
            1,
            text.as_ptr() as u64,
            length,
            output,
            capacity,
            required as *mut u64 as u64,
        )
    };
    match call(0, 0, &mut required) {
        0 if required == 0 => return Ok(Vec::new()),
        3 if required > 0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(required).map_err(|_| MojoError::Capacity)?;
    if count > text.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut spans = vec![[u64::MAX; 2]; count];
    let capacity = i64::try_from(count).map_err(|_| MojoError::Capacity)?;
    match call(spans.as_mut_ptr() as u64, capacity, &mut required) {
        0 if required == count as u64 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let mut previous_end = None;
    spans
        .into_iter()
        .map(|[start, end]| {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            if previous_end.is_some_and(|last| start <= last) {
                return Err(MojoError::InvalidOutput);
            }
            previous_end = Some(end);
            text.get(start..end).ok_or(MojoError::InvalidOutput)
        })
        .collect()
}

#[cfg(test)]
#[path = "provider_sse_data_tests.rs"]
mod tests;
