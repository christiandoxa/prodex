use super::*;

pub const RUNTIME_ERROR_MODE_CODE_QUOTA: i64 = 12;
pub const RUNTIME_ERROR_MODE_CODE_RATE: i64 = 13;
pub const RUNTIME_ERROR_MODE_CODE_OVERLOAD: i64 = 14;
pub const RUNTIME_ERROR_MODE_HTTP: i64 = 0;
pub const RUNTIME_ERROR_MODE_STREAM: i64 = 1;
pub const RUNTIME_ERROR_MODE_TEXT_QUOTA: i64 = 6;
pub const RUNTIME_ERROR_MODE_TEXT_AUTHORITATIVE_QUOTA: i64 = 7;
pub const RUNTIME_ERROR_MODE_TEXT_RATE: i64 = 8;
pub const RUNTIME_ERROR_MODE_TEXT_PROFILE: i64 = 9;
pub const RUNTIME_ERROR_MODE_TEXT_OVERLOAD: i64 = 10;
pub const RUNTIME_ERROR_MODE_TEXT_WORKSPACE: i64 = 11;

unsafe extern "C" {
    fn prodex_mojo_rich_model_fallback_v2(
        abi_version: i64,
        provider: u64,
        model: u64,
        output_records: u64,
        record_capacity: i64,
        output: u64,
        output_capacity: i64,
        hash_slots: u64,
        hash_capacity: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_rich_model_fallback_plan_v1(
        abi_version: i64,
        provider: u64,
        models: u64,
        model_count: i64,
        output_records: u64,
        record_capacity: i64,
        output: u64,
        output_capacity: i64,
        hash_slots: u64,
        hash_capacity: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_rich_runtime_error_policy_v1(
        abi_version: i64,
        operation: i64,
        status: i64,
        phase: i64,
        body: u64,
        body_len: i64,
        output_records: u64,
        record_capacity: i64,
        output: u64,
        output_capacity: i64,
        result: u64,
    ) -> i64;
}

impl MojoError {
    /// Runs the bounded runtime error/payload classifier through the rich Mojo ABI.
    ///
    /// The tuple is `(class_tag, action_tag, message)`. Input acquisition,
    /// retry-after parsing, and forwarding stay in their Rust owners; this
    /// method only adapts caller-owned bytes to the deterministic kernel.
    pub fn rich_runtime_error_policy(
        operation: i64,
        status: u16,
        phase: i64,
        body: &[u8],
    ) -> Result<(i64, i64, String), Self> {
        ensure_rich_abi()?;
        const MAX_RUNTIME_ERROR_BYTES: usize = 65_536;
        let body = if body.len() <= MAX_RUNTIME_ERROR_BYTES && std::str::from_utf8(body).is_ok() {
            body
        } else {
            &[]
        };
        let mut records = [RichFallbackRecord::default()];
        let mut output = vec![0_u8; body.len().saturating_add(256).max(256)];
        let mut result = RichFallbackResult::default();
        let status = unsafe {
            prodex_mojo_rich_runtime_error_policy_v1(
                RICH_ABI_VERSION,
                operation,
                i64::from(status),
                phase,
                mojo_pointer_address(body.as_ptr()),
                i64::try_from(body.len()).map_err(|_| Self::InvalidInput)?,
                mojo_pointer_address(records.as_mut_ptr()),
                1,
                mojo_pointer_address(output.as_mut_ptr()),
                i64::try_from(output.len()).map_err(|_| Self::InvalidInput)?,
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
            || result.records_written > 1
            || result.output_written < 0
            || result.output_written as usize > output.len()
        {
            return Err(Self::InvalidOutput);
        }
        if result.records_written == 0 {
            return Ok((0, 0, String::new()));
        }
        let record = records[0];
        if !(0..=5).contains(&record.source_kind) || !(0..=2).contains(&record.input_index) {
            return Err(Self::InvalidOutput);
        }
        let output = &output[..result.output_written as usize];
        let message = std::str::from_utf8(slice(output, record.model)?)
            .map_err(|_| Self::InvalidOutput)?
            .to_string();
        Ok((record.source_kind, record.input_index, message))
    }
}

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
    let provider_view = view(provider);
    let model_view = view(model);
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
