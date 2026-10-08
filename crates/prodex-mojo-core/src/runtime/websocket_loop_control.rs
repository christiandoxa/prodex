use crate::MojoError;

const WEBSOCKET_LOOP_CONTROL_ABI_VERSION: i64 = 1;
const WEBSOCKET_LOOP_CONTROL_FIELD_COUNT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketPrecommitBudgetInput {
    pub fresh_retry_pending: bool,
    pub budget_exhausted: bool,
    pub attempts: usize,
    pub continuation: bool,
    pub saw_overload_failure: bool,
    pub saw_rate_limit_failure: bool,
    pub saw_transport_failure: bool,
    pub route_has_retryable_profile: bool,
    pub recovery_sweeps: usize,
    pub profile_count: usize,
    pub attempt_limit: usize,
}

unsafe extern "C" {
    fn prodex_runtime_websocket_precommit_exhausted_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        output_address: u64,
    ) -> i64;
}

/// Applies websocket-specific precommit retry exhaustion after Rust gathers runtime state.
pub fn websocket_precommit_budget_exhausted(
    input: WebsocketPrecommitBudgetInput,
) -> Result<bool, MojoError> {
    let usize_to_u64 = |value| u64::try_from(value).unwrap_or(u64::MAX);
    let fields = [
        u64::from(input.fresh_retry_pending),
        u64::from(input.budget_exhausted),
        usize_to_u64(input.attempts),
        u64::from(input.continuation),
        u64::from(input.saw_overload_failure),
        u64::from(input.saw_rate_limit_failure),
        u64::from(input.saw_transport_failure),
        u64::from(input.route_has_retryable_profile),
        usize_to_u64(input.recovery_sweeps),
        usize_to_u64(if input.recovery_sweeps == 0 {
            input.profile_count
        } else {
            input.attempt_limit
        }),
    ];
    let mut output = i64::MIN;
    let status = unsafe {
        prodex_runtime_websocket_precommit_exhausted_v1(
            WEBSOCKET_LOOP_CONTROL_ABI_VERSION,
            fields.as_ptr() as usize as u64,
            i64::try_from(WEBSOCKET_LOOP_CONTROL_FIELD_COUNT)
                .map_err(|_| MojoError::InvalidInput)?,
            &mut output as *mut i64 as usize as u64,
        )
    };
    match status {
        0 => match output {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}
