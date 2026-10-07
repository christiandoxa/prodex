use super::*;

/// Apply the authoritative Kiro request capability policy in Mojo.
pub fn kiro_validate_request_json(
    mode: KiroRequestValidationMode,
    input: &str,
    allow_token_limit: bool,
) -> Result<KiroRequestValidationPlan, MojoError> {
    ensure_rich_abi()?;
    if input.len() > KIRO_KERNEL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; 3];
    let status = unsafe {
        prodex_mojo_kiro_request_validation_json_v1(
            RICH_ABI_VERSION,
            mode as i64,
            input.as_ptr() as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(allow_token_limit),
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    if !(KiroRequestValidationPlan::REASON_NONE
        ..=KiroRequestValidationPlan::REASON_REASONING_EFFORT)
        .contains(&output[0])
        || !(-1..=2).contains(&output[1])
        || !(0..=1).contains(&output[2])
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(KiroRequestValidationPlan {
        reason: output[0],
        detail: output[1],
        detail_is_invalid: output[2] == 1,
    })
}

pub fn kiro_validate_request(
    input: KiroRequestValidationInput,
) -> Result<KiroRequestValidationPlan, MojoError> {
    ensure_rich_abi()?;
    if input.flags & !KiroRequestValidationInput::FLAG_MASK != 0
        || !(-1..=2).contains(&input.detail)
    {
        return Err(MojoError::InvalidInput);
    }
    let ffi_input = KiroRequestValidationFfiInput {
        mode: input.mode as i64,
        flags: input.flags,
        detail: input.detail,
        allow_token_limit: i64::from(input.allow_token_limit),
    };
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_mojo_kiro_request_validation_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            4 => MojoError::AbiMismatch,
            1 => MojoError::InvalidInput,
            _ => MojoError::InvalidOutput,
        });
    }
    if !(KiroRequestValidationPlan::REASON_NONE
        ..=KiroRequestValidationPlan::REASON_REASONING_EFFORT)
        .contains(&output[0])
        || !(-1..=2).contains(&output[1])
        || !matches!(output[2], 0 | 1)
        || (matches!(
            output[0],
            KiroRequestValidationPlan::REASON_TOKEN_LIMIT
                | KiroRequestValidationPlan::REASON_GENERATION_CONTROL
        ) && output[1] < 0)
        || (!matches!(
            output[0],
            KiroRequestValidationPlan::REASON_TOKEN_LIMIT
                | KiroRequestValidationPlan::REASON_GENERATION_CONTROL
        ) && output[1] != -1)
        || (!matches!(
            output[0],
            KiroRequestValidationPlan::REASON_TOKEN_LIMIT
                | KiroRequestValidationPlan::REASON_LOGPROBS
        ) && output[2] != 0)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(KiroRequestValidationPlan {
        reason: output[0],
        detail: output[1],
        detail_is_invalid: output[2] == 1,
    })
}
