use super::*;

pub fn model_fallback_chain(provider: &str, model: &str) -> Result<Vec<String>, MojoError> {
    ensure_rich_abi()?;
    let record_capacity = 32_usize;
    let scratch_capacity = hash_capacity(record_capacity)?;
    let output_capacity = model
        .len()
        .checked_add(4_096)
        .ok_or(MojoError::InvalidInput)?;
    let mut records = vec![RichFallbackRecord::default(); record_capacity];
    let mut output = vec![0_u8; output_capacity];
    let mut hash_slots = vec![-1_i64; scratch_capacity];
    let mut result = RichFallbackResult::default();
    let status = unsafe {
        prodex_mojo_rich_model_fallback_v2(
            RICH_ABI_VERSION,
            view(provider),
            view(model),
            records.as_mut_ptr(),
            i64::try_from(record_capacity).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr(),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            hash_slots.as_mut_ptr(),
            i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
            &mut result,
        )
    };
    if status != 0 {
        return Err(status_error(
            status,
            4,
            result.issue_kind,
            result.issue_offset,
            result.issue_length,
        ));
    }
    if result.records_written < 0
        || result.records_written as usize > record_capacity
        || result.output_written < 0
        || result.output_written as usize > output.len()
    {
        return Err(MojoError::InvalidOutput);
    }
    let output = &output[..result.output_written as usize];
    records[..result.records_written as usize]
        .iter()
        .map(|record| {
            Ok(std::str::from_utf8(slice(output, record.model)?)
                .map_err(|_| MojoError::InvalidOutput)?
                .to_string())
        })
        .collect()
}
