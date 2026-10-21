use super::{
    ABI_VERSION, BrokerBinaryIdentityView, BrokerVersionGuardOutcome, BrokerVersionGuardPlan,
    MojoError, identity_parts, length, prodex_runtime_broker_guard_plan_v1,
    prodex_runtime_broker_parse_version_v1, ptr, replacement_reason_from_code,
};

pub fn version_guard_plan(
    process_alive: bool,
    current_binary: BrokerBinaryIdentityView<'_>,
    current_version: BrokerBinaryIdentityView<'_>,
    observed: BrokerBinaryIdentityView<'_>,
    active_requests: usize,
    live_leases: usize,
) -> Result<BrokerVersionGuardPlan, MojoError> {
    let (
        binary_flags,
        binary_version_address,
        binary_version_length,
        binary_sha_address,
        binary_sha_length,
    ) = identity_parts(current_binary)?;
    let (
        version_flags,
        version_version_address,
        version_version_length,
        version_sha_address,
        version_sha_length,
    ) = identity_parts(current_version)?;
    let (
        observed_flags,
        observed_version_address,
        observed_version_length,
        observed_sha_address,
        observed_sha_length,
    ) = identity_parts(observed)?;
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_runtime_broker_guard_plan_v1(
            ABI_VERSION,
            i64::from(process_alive),
            binary_flags,
            binary_version_address,
            binary_version_length,
            binary_sha_address,
            binary_sha_length,
            version_flags,
            version_version_address,
            version_version_length,
            version_sha_address,
            version_sha_length,
            observed_flags,
            observed_version_address,
            observed_version_length,
            observed_sha_address,
            observed_sha_length,
            u64::try_from(active_requests).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(live_leases).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidOutput);
    }
    let outcome = match output[0] {
        0 => BrokerVersionGuardOutcome::Compatible,
        1 => BrokerVersionGuardOutcome::DeferredActiveRequests,
        2 => BrokerVersionGuardOutcome::Replaced,
        _ => return Err(MojoError::InvalidOutput),
    };
    let use_version_identity = match output[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let replacement_reason = match output[2] {
        0 => None,
        value => Some(replacement_reason_from_code(value)?),
    };
    Ok(BrokerVersionGuardPlan {
        outcome,
        use_version_identity,
        replacement_reason,
    })
}

pub fn parse_prodex_version(output: &str) -> Result<Option<&str>, MojoError> {
    let mut span = [-1_i64; 2];
    let status = unsafe {
        prodex_runtime_broker_parse_version_v1(
            ABI_VERSION,
            ptr(output),
            length(output)?,
            span.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(None),
        1 => {
            let start = usize::try_from(span[0]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(span[1]).map_err(|_| MojoError::InvalidOutput)?;
            output
                .get(start..end)
                .map(Some)
                .ok_or(MojoError::InvalidOutput)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}
