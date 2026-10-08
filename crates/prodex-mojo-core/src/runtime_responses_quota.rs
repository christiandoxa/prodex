//! Mojo-owned Responses quota replay eligibility.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_runtime_responses_quota_turn_state_replay_v1(
        abi_version: i64,
        previous_response_present: i64,
        turn_state_present: i64,
        turn_state_owner_matches: i64,
        compact_followup_present: i64,
        reconstructable_full_history: i64,
    ) -> i64;
}

/// Whether a quota-blocked turn-state request can safely replay from full history.
pub fn turn_state_full_context_replay_candidate(
    previous_response_present: bool,
    turn_state_present: bool,
    turn_state_owner_matches: bool,
    compact_followup_present: bool,
    reconstructable_full_history: bool,
) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_responses_quota_turn_state_replay_v1(
            ABI_VERSION,
            i64::from(previous_response_present),
            i64::from(turn_state_present),
            i64::from(turn_state_owner_matches),
            i64::from(compact_followup_present),
            i64::from(reconstructable_full_history),
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_requires_owned_turn_state_and_reconstructable_history() {
        assert!(turn_state_full_context_replay_candidate(false, true, true, false, true).unwrap());
        for input in [
            [true, true, true, false, true],
            [false, false, true, false, true],
            [false, true, false, false, true],
            [false, true, true, true, true],
            [false, true, true, false, false],
        ] {
            assert!(
                !turn_state_full_context_replay_candidate(
                    input[0], input[1], input[2], input[3], input[4]
                )
                .unwrap(),
                "replay must fail closed for {input:?}"
            );
        }
    }

    #[test]
    fn abi_rejects_unknown_version_and_non_boolean_input() {
        assert_eq!(
            unsafe { prodex_runtime_responses_quota_turn_state_replay_v1(0, 0, 1, 1, 0, 1) },
            -4
        );
        assert_eq!(
            unsafe {
                prodex_runtime_responses_quota_turn_state_replay_v1(ABI_VERSION, 2, 1, 1, 0, 1)
            },
            -1
        );
    }
}
