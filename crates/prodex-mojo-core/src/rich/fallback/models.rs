use super::{
    MojoError, RICH_ABI_VERSION, RICH_STATUS_CAPACITY, RichFallbackRecord, RichFallbackResult,
    ensure_rich_abi, hash_capacity, mojo_mut_pointer_address, mojo_pointer_address,
    prodex_mojo_rich_gemini_code_assist_model_allowed_v1, prodex_mojo_rich_model_fallback_head_v1,
    prodex_mojo_rich_model_fallback_plan_v1, prodex_mojo_rich_model_fallback_v2,
    prodex_mojo_rich_model_session_memory_allowed_v1, slice, status_error, view,
};

pub fn model_fallback_chain(provider: &str, model: &str) -> Result<Vec<String>, MojoError> {
    ensure_rich_abi()?;
    let output_capacity = model
        .len()
        .checked_add(4_096)
        .ok_or(MojoError::InvalidInput)?;
    let provider_view = view(provider);
    let model_view = view(model);
    let max_records = model
        .len()
        .div_ceil(2)
        .max(8)
        .checked_next_power_of_two()
        .ok_or(MojoError::InvalidInput)?
        .max(32);
    let mut record_capacity = 32_usize;
    let (records, output, result) = loop {
        let scratch_capacity = hash_capacity(record_capacity)?;
        let mut records = vec![RichFallbackRecord::default(); record_capacity];
        let mut output = vec![0_u8; output_capacity];
        let mut hash_slots = vec![-1_i64; scratch_capacity];
        let mut result = RichFallbackResult::default();
        let status = unsafe {
            prodex_mojo_rich_model_fallback_v2(
                RICH_ABI_VERSION,
                mojo_pointer_address(&provider_view),
                mojo_pointer_address(&model_view),
                mojo_pointer_address(records.as_mut_ptr()),
                i64::try_from(record_capacity).map_err(|_| MojoError::InvalidInput)?,
                mojo_pointer_address(output.as_mut_ptr()),
                i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_pointer_address(hash_slots.as_mut_ptr()),
                i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(&mut result),
            )
        };
        if status == RICH_STATUS_CAPACITY
            && let Ok(required_records) = usize::try_from(result.required_records)
            && required_records > record_capacity
        {
            let next_capacity = required_records
                .checked_next_power_of_two()
                .ok_or(MojoError::InvalidInput)?;
            if next_capacity <= max_records {
                record_capacity = next_capacity;
                continue;
            }
        }
        if status != 0 {
            return Err(status_error(
                status,
                4,
                result.issue_kind,
                result.issue_offset,
                result.issue_length,
            ));
        }
        break (records, output, result);
    };
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

pub fn model_fallback_head(provider: &str, model: &str) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let output_capacity = model
        .len()
        .checked_add(4_096)
        .ok_or(MojoError::InvalidInput)?;
    let provider_view = view(provider);
    let model_view = view(model);
    let max_records = model
        .len()
        .div_ceil(2)
        .max(8)
        .checked_next_power_of_two()
        .ok_or(MojoError::InvalidInput)?
        .max(32);
    let mut record_capacity = 32_usize;
    let (records, output, result) = loop {
        let scratch_capacity = hash_capacity(record_capacity)?;
        let mut records = vec![RichFallbackRecord::default(); record_capacity];
        let mut output = vec![0_u8; output_capacity];
        let mut hash_slots = vec![-1_i64; scratch_capacity];
        let mut result = RichFallbackResult::default();
        let status = unsafe {
            prodex_mojo_rich_model_fallback_head_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&provider_view),
                mojo_pointer_address(&model_view),
                mojo_pointer_address(records.as_mut_ptr()),
                i64::try_from(record_capacity).map_err(|_| MojoError::InvalidInput)?,
                mojo_pointer_address(output.as_mut_ptr()),
                i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_pointer_address(hash_slots.as_mut_ptr()),
                i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(&mut result),
            )
        };
        if status == RICH_STATUS_CAPACITY
            && let Ok(required_records) = usize::try_from(result.required_records)
            && required_records > record_capacity
        {
            let next_capacity = required_records
                .checked_next_power_of_two()
                .ok_or(MojoError::InvalidInput)?;
            if next_capacity <= max_records {
                record_capacity = next_capacity;
                continue;
            }
        }
        if status != 0 {
            return Err(status_error(
                status,
                4,
                result.issue_kind,
                result.issue_offset,
                result.issue_length,
            ));
        }
        break (records, output, result);
    };
    if result.records_written < 0
        || result.records_written as usize > record_capacity
        || result.output_written < 0
        || result.output_written as usize > output.len()
    {
        return Err(MojoError::InvalidOutput);
    }
    let output = &output[..result.output_written as usize];
    if result.records_written > 0 {
        let record = records.first().ok_or(MojoError::InvalidOutput)?;
        return std::str::from_utf8(slice(output, record.model)?)
            .map(str::to_string)
            .map_err(|_| MojoError::InvalidOutput);
    }
    std::str::from_utf8(output)
        .map(str::to_string)
        .map_err(|_| MojoError::InvalidOutput)
}

pub fn gemini_code_assist_model_allowed(model: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    i64::try_from(model.len()).map_err(|_| MojoError::InvalidInput)?;
    let model_view = view(model);
    let mut allowed = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_gemini_code_assist_model_allowed_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&model_view),
            mojo_mut_pointer_address(&mut allowed),
        )
    };
    if status != 0 {
        return Err(status_error(status, 4, 0, -1, 0));
    }
    match allowed {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn model_session_memory_allowed(model: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let model_view = view(model);
    let mut allowed = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_model_session_memory_allowed_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&model_view),
            mojo_mut_pointer_address(&mut allowed),
        )
    };
    if status != 0 {
        return Err(status_error(status, 4, 0, -1, 0));
    }
    match allowed {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn model_fallback_plan(provider: &str, models: &[&str]) -> Result<Vec<String>, MojoError> {
    ensure_rich_abi()?;
    if models.len() > 256 {
        return Err(MojoError::InvalidInput);
    }
    let record_capacity = 2_048_usize;
    let scratch_capacity = hash_capacity(record_capacity)?;
    let output_capacity = 4_096_usize
        .checked_add(
            models
                .len()
                .checked_mul(4_096)
                .ok_or(MojoError::InvalidInput)?,
        )
        .ok_or(MojoError::InvalidInput)?;
    let mut records = vec![RichFallbackRecord::default(); record_capacity];
    let mut output = vec![0_u8; output_capacity];
    let mut hash_slots = vec![-1_i64; scratch_capacity];
    let mut result = RichFallbackResult::default();
    let provider_view = view(provider);
    let model_views = models.iter().map(|model| view(model)).collect::<Vec<_>>();
    let status = unsafe {
        prodex_mojo_rich_model_fallback_plan_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&provider_view),
            mojo_pointer_address(model_views.as_ptr()),
            i64::try_from(model_views.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(records.as_mut_ptr()),
            i64::try_from(record_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(hash_slots.as_mut_ptr()),
            i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
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
