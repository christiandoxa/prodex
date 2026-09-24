use super::*;

pub const RUNTIME_ERROR_MODE_CODE_QUOTA: i64 = 12;
pub const RUNTIME_ERROR_MODE_CODE_RATE: i64 = 13;
pub const RUNTIME_ERROR_MODE_CODE_OVERLOAD: i64 = 14;
pub const RUNTIME_ERROR_MODE_HTTP: i64 = 0;
pub const RUNTIME_ERROR_MODE_STREAM: i64 = 1;
pub const RUNTIME_ERROR_MODE_JSON_QUOTA: i64 = 2;
pub const RUNTIME_ERROR_MODE_JSON_RATE: i64 = 3;
pub const RUNTIME_ERROR_MODE_JSON_PROFILE: i64 = 4;
pub const RUNTIME_ERROR_MODE_JSON_OVERLOAD: i64 = 5;
pub const RUNTIME_ERROR_MODE_TEXT_QUOTA: i64 = 6;
pub const RUNTIME_ERROR_MODE_TEXT_AUTHORITATIVE_QUOTA: i64 = 7;
pub const RUNTIME_ERROR_MODE_TEXT_RATE: i64 = 8;
pub const RUNTIME_ERROR_MODE_TEXT_PROFILE: i64 = 9;
pub const RUNTIME_ERROR_MODE_TEXT_OVERLOAD: i64 = 10;
pub const RUNTIME_ERROR_MODE_TEXT_WORKSPACE: i64 = 11;
pub const RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS: i64 = 0;
pub const RUNTIME_RETRY_AFTER_MODE_DURATION_MILLIS: i64 = 1;
pub const RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS: i64 = 2;
const RUNTIME_RETRY_AFTER_CAP_MILLIS: i64 = 300_000;
pub const PREVIOUS_RESPONSE_ERROR_MODE_STRUCTURED: i64 = 0;
pub const PREVIOUS_RESPONSE_ERROR_MODE_TEXT: i64 = 1;
pub const PREVIOUS_RESPONSE_ERROR_CLASS_NONE: i64 = 0;
pub const PREVIOUS_RESPONSE_ERROR_CLASS_NOT_FOUND: i64 = 1;
pub const PREVIOUS_RESPONSE_ERROR_CLASS_INVALID_ID: i64 = 2;
pub const PREVIOUS_RESPONSE_ERROR_CLASS_TOOL_CONTEXT: i64 = 3;
const PREVIOUS_RESPONSE_ERROR_MAX_BYTES: usize = 65_536;
pub const PREVIOUS_RESPONSE_PLAN_OUTPUT_COUNT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviousResponsePlanInput {
    pub route: i64,
    pub previous_response_present: bool,
    pub has_turn_state_retry: bool,
    pub request_requires_previous_response_affinity: bool,
    pub trusted_previous_response_affinity: bool,
    pub request_turn_state_present: bool,
    pub previous_response_fresh_fallback_used: bool,
    pub fresh_fallback_shape: i64,
    pub retry_index: usize,
    pub has_session_affinity: bool,
}

impl Default for PreviousResponsePlanInput {
    fn default() -> Self {
        Self {
            route: 0,
            previous_response_present: false,
            has_turn_state_retry: false,
            request_requires_previous_response_affinity: false,
            trusted_previous_response_affinity: false,
            request_turn_state_present: false,
            previous_response_fresh_fallback_used: false,
            fresh_fallback_shape: -1,
            retry_index: 0,
            has_session_affinity: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviousResponsePlan {
    pub retry_reason: i64,
    pub retry_delay_ms: Option<u64>,
    pub chain_reason: i64,
    pub request_requires_locked_affinity: bool,
    pub stale_policy: i64,
    pub fresh_fail_closed: bool,
    pub fresh_blocked_without_affinity: bool,
    pub observability: i64,
    pub effective_shape: i64,
    pub websocket_requires_affinity: bool,
}

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
    fn prodex_mojo_rich_retry_after_millis_v1(
        abi_version: i64,
        mode: i64,
        number_address: u64,
        number_length: i64,
    ) -> i64;
    fn prodex_mojo_previous_response_error_class_v1(
        abi_version: i64,
        mode: i64,
        error_type_address: u64,
        error_type_length: i64,
        code_address: u64,
        code_length: i64,
        param_address: u64,
        param_length: i64,
        message_address: u64,
        message_length: i64,
    ) -> i64;
    fn prodex_mojo_rate_limit_header_class_v1(address: u64, length: i64) -> i64;
    fn prodex_runtime_previous_response_plan_v1(
        route: i64,
        previous_response_present: i64,
        has_turn_state_retry: i64,
        request_requires_previous_response_affinity: i64,
        trusted_previous_response_affinity: i64,
        request_turn_state_present: i64,
        previous_response_fresh_fallback_used: i64,
        fresh_fallback_shape: i64,
        retry_index: i64,
        has_session_affinity: i64,
        output: *mut i64,
    ) -> i64;
}

pub const RATE_LIMIT_HEADER_CLASS_NONE: i64 = 0;
pub const RATE_LIMIT_HEADER_CLASS_RATE: i64 = 1;
pub const RATE_LIMIT_HEADER_CLASS_QUOTA: i64 = 2;

pub fn rate_limit_header_class(value: &str) -> Result<i64, MojoError> {
    ensure_rich_abi()?;
    let result = unsafe {
        prodex_mojo_rate_limit_header_class_v1(
            mojo_pointer_address(value.as_ptr()),
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        RATE_LIMIT_HEADER_CLASS_NONE
        | RATE_LIMIT_HEADER_CLASS_RATE
        | RATE_LIMIT_HEADER_CLASS_QUOTA => Ok(result),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_retry_after_millis(mode: i64, number: &str) -> Result<Option<u64>, MojoError> {
    ensure_rich_abi()?;
    if !(RUNTIME_RETRY_AFTER_MODE_HEADER_SECONDS..=RUNTIME_RETRY_AFTER_MODE_DURATION_SECONDS)
        .contains(&mode)
        || number.is_empty()
    {
        return Err(MojoError::InvalidInput);
    }
    let value = unsafe {
        prodex_mojo_rich_retry_after_millis_v1(
            RICH_ABI_VERSION,
            mode,
            mojo_pointer_address(number.as_ptr()),
            i64::try_from(number.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match value {
        -1 => Ok(None),
        1..=RUNTIME_RETRY_AFTER_CAP_MILLIS => Ok(Some(value as u64)),
        -2 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn previous_response_error_text_parts(value: Option<&str>) -> Result<(u64, i64), MojoError> {
    match value {
        Some(value) if value.len() <= PREVIOUS_RESPONSE_ERROR_MAX_BYTES => Ok((
            mojo_pointer_address(value.as_ptr()),
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        )),
        Some(_) => Err(MojoError::InvalidInput),
        None => Ok((0, 0)),
    }
}

pub fn previous_response_error_class(
    mode: i64,
    error_type: Option<&str>,
    code: Option<&str>,
    param: Option<&str>,
    message: Option<&str>,
) -> Result<i64, MojoError> {
    ensure_rich_abi()?;
    if !(PREVIOUS_RESPONSE_ERROR_MODE_STRUCTURED..=PREVIOUS_RESPONSE_ERROR_MODE_TEXT)
        .contains(&mode)
    {
        return Err(MojoError::InvalidInput);
    }
    let (error_type_address, error_type_length) = previous_response_error_text_parts(error_type)?;
    let (code_address, code_length) = previous_response_error_text_parts(code)?;
    let (param_address, param_length) = previous_response_error_text_parts(param)?;
    let (message_address, message_length) = previous_response_error_text_parts(message)?;
    let value = unsafe {
        prodex_mojo_previous_response_error_class_v1(
            RICH_ABI_VERSION,
            mode,
            error_type_address,
            error_type_length,
            code_address,
            code_length,
            param_address,
            param_length,
            message_address,
            message_length,
        )
    };
    match value {
        PREVIOUS_RESPONSE_ERROR_CLASS_NONE
        | PREVIOUS_RESPONSE_ERROR_CLASS_NOT_FOUND
        | PREVIOUS_RESPONSE_ERROR_CLASS_INVALID_ID
        | PREVIOUS_RESPONSE_ERROR_CLASS_TOOL_CONTEXT => Ok(value),
        -2 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn previous_response_plan(
    input: PreviousResponsePlanInput,
) -> Result<PreviousResponsePlan, MojoError> {
    ensure_rich_abi()?;
    if !(0..=1).contains(&input.route) || !(-1..=3).contains(&input.fresh_fallback_shape) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; PREVIOUS_RESPONSE_PLAN_OUTPUT_COUNT];
    let status = unsafe {
        prodex_runtime_previous_response_plan_v1(
            input.route,
            i64::from(input.previous_response_present),
            i64::from(input.has_turn_state_retry),
            i64::from(input.request_requires_previous_response_affinity),
            i64::from(input.trusted_previous_response_affinity),
            i64::from(input.request_turn_state_present),
            i64::from(input.previous_response_fresh_fallback_used),
            input.fresh_fallback_shape,
            i64::try_from(input.retry_index).map_err(|_| MojoError::InvalidInput)?,
            i64::from(input.has_session_affinity),
            output.as_mut_ptr(),
        )
    };
    if status != 0
        || !(0..=2).contains(&output[0])
        || output[1] < -1
        || !(0..=2).contains(&output[2])
        || !matches!(output[3], 0 | 1)
        || !(0..=2).contains(&output[4])
        || !matches!(output[5], 0 | 1)
        || !matches!(output[6], 0 | 1)
        || !(0..=2).contains(&output[7])
        || !(-1..=3).contains(&output[8])
        || !matches!(output[9], 0 | 1)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(PreviousResponsePlan {
        retry_reason: output[0],
        retry_delay_ms: (output[1] >= 0).then(|| output[1] as u64),
        chain_reason: output[2],
        request_requires_locked_affinity: output[3] == 1,
        stale_policy: output[4],
        fresh_fail_closed: output[5] == 1,
        fresh_blocked_without_affinity: output[6] == 1,
        observability: output[7],
        effective_shape: output[8],
        websocket_requires_affinity: output[9] == 1,
    })
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
