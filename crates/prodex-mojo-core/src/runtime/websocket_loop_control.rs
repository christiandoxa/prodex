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

#[cfg(test)]
mod tests {
    use super::*;

    // Test-only specification, independent from ABI implementation. Not a
    // runtime fallback: production invokes only the Mojo policy.
    fn expected(input: WebsocketPrecommitBudgetInput) -> bool {
        if input.fresh_retry_pending {
            return false;
        }
        if !input.continuation
            && (input.saw_overload_failure
                || input.saw_rate_limit_failure
                || input.saw_transport_failure)
            && input.route_has_retryable_profile
        {
            return false;
        }
        if input.recovery_sweeps == 0 {
            if input.saw_transport_failure
                && !input.continuation
                && !input.budget_exhausted
                && input.attempts < input.profile_count
            {
                return false;
            }
            return input.budget_exhausted;
        }
        input.attempts >= input.attempt_limit
    }

    #[test]
    fn precommit_budget_mojo_preserves_retry_precedence_all_boolean_states() {
        for flags in 0_u8..128 {
            for sweeps in [0, 1, 2] {
                for attempts in [0, 1, 2, 3, 5, 9, usize::MAX] {
                    for bound in [0, 1, 2, 4, 8, usize::MAX] {
                        let input = WebsocketPrecommitBudgetInput {
                            fresh_retry_pending: flags & 1 != 0,
                            budget_exhausted: flags & 2 != 0,
                            attempts,
                            continuation: flags & 4 != 0,
                            saw_overload_failure: flags & 8 != 0,
                            saw_rate_limit_failure: flags & 16 != 0,
                            saw_transport_failure: flags & 32 != 0,
                            route_has_retryable_profile: flags & 64 != 0,
                            recovery_sweeps: sweeps,
                            profile_count: bound,
                            attempt_limit: bound,
                        };
                        assert_eq!(
                            websocket_precommit_budget_exhausted(input),
                            Ok(expected(input)),
                            "flags={flags:07b}, sweeps={sweeps}, attempts={attempts}, bound={bound}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn precommit_budget_rejects_nonboolean_abi_fields() {
        let mut fields = [0_u64; WEBSOCKET_LOOP_CONTROL_FIELD_COUNT];
        let mut output = -1_i64;
        for index in [0, 1, 3, 4, 5, 6, 7] {
            fields[index] = 2;
            assert_eq!(
                unsafe {
                    prodex_runtime_websocket_precommit_exhausted_v1(
                        WEBSOCKET_LOOP_CONTROL_ABI_VERSION,
                        fields.as_ptr() as usize as u64,
                        WEBSOCKET_LOOP_CONTROL_FIELD_COUNT as i64,
                        &mut output as *mut i64 as usize as u64,
                    )
                },
                1,
                "nonboolean field {index} must fail closed",
            );
            fields[index] = 0;
        }
        assert_eq!(
            unsafe {
                prodex_runtime_websocket_precommit_exhausted_v1(
                    0,
                    fields.as_ptr() as usize as u64,
                    WEBSOCKET_LOOP_CONTROL_FIELD_COUNT as i64,
                    &mut output as *mut i64 as usize as u64,
                )
            },
            4,
        );
    }
}
