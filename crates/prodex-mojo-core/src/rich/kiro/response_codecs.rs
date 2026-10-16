use super::{
    KIRO_RESPONSE_MAX_BYTES, KIRO_RESPONSE_MAX_OUTPUT_BYTES, KiroChatResponseInputPlan, MojoError,
    RICH_ABI_VERSION, ensure_rich_abi, mojo_mut_pointer_address,
    prodex_mojo_kiro_anthropic_sse_body_v1, prodex_mojo_kiro_chat_response_input_plan_v1,
    prodex_mojo_kiro_chat_response_rewrite_v1,
};

/// Render one canonical Kiro Anthropic message as an Anthropic SSE body.
pub fn kiro_anthropic_sse_body(input: &str) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    if input.len() > KIRO_RESPONSE_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let capacity = input
        .len()
        .checked_mul(8)
        .and_then(|value| value.checked_add(4096))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(capacity, 0);
    loop {
        let mut written = 0_i64;
        let status = unsafe {
            prodex_mojo_kiro_anthropic_sse_body_v1(
                RICH_ABI_VERSION,
                input.as_ptr() as u64,
                i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(&mut written),
            )
        };
        if status == 3 {
            let capacity = output
                .len()
                .checked_mul(2)
                .map(|capacity| capacity.min(KIRO_RESPONSE_MAX_OUTPUT_BYTES))
                .filter(|capacity| *capacity > output.len())
                .ok_or(MojoError::Capacity)?;
            output
                .try_reserve_exact(capacity - output.len())
                .map_err(|_| MojoError::Capacity)?;
            output.resize(capacity, 0);
            continue;
        }
        if status != 0 {
            return Err(match status {
                1 | 2 => MojoError::InvalidInput,
                4 => MojoError::AbiMismatch,
                _ => MojoError::InvalidOutput,
            });
        }
        let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
        if written > output.len() {
            return Err(MojoError::InvalidOutput);
        }
        output.truncate(written);
        return Ok(output);
    }
}

pub fn kiro_chat_response_input_plan(input: &[u8]) -> Result<KiroChatResponseInputPlan, MojoError> {
    ensure_rich_abi()?;
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_kiro_chat_response_input_plan_v1(
            RICH_ABI_VERSION,
            input.as_ptr() as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    };
    match status {
        0 => {}
        1 | 2 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match output {
        0 => Ok(KiroChatResponseInputPlan::Valid),
        1 => Ok(KiroChatResponseInputPlan::TooLarge),
        2 => Ok(KiroChatResponseInputPlan::InvalidJson),
        3 => Ok(KiroChatResponseInputPlan::InvalidResponseBody),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Rewrite one canonical Kiro Responses JSON value into Chat Completions JSON.
pub fn kiro_rewrite_chat_response_json(input: &str, request_id: u64) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    if input.len() > KIRO_RESPONSE_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let capacity = input
        .len()
        .checked_add(4096)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(capacity, 0);
    loop {
        let mut written = 0_i64;
        let status = unsafe {
            prodex_mojo_kiro_chat_response_rewrite_v1(
                RICH_ABI_VERSION,
                input.as_ptr() as u64,
                i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
                request_id,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(&mut written),
            )
        };
        if status == 3 {
            let capacity = output
                .len()
                .checked_mul(2)
                .map(|capacity| capacity.min(KIRO_RESPONSE_MAX_OUTPUT_BYTES))
                .filter(|capacity| *capacity > output.len())
                .ok_or(MojoError::Capacity)?;
            output
                .try_reserve_exact(capacity - output.len())
                .map_err(|_| MojoError::Capacity)?;
            output.resize(capacity, 0);
            continue;
        }
        if status != 0 {
            return Err(match status {
                1 | 2 => MojoError::InvalidInput,
                4 => MojoError::AbiMismatch,
                _ => MojoError::InvalidOutput,
            });
        }
        let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
        if written > output.len() {
            return Err(MojoError::InvalidOutput);
        }
        output.truncate(written);
        return Ok(output);
    }
}
