//! Native wire fields acquired by Serde; classification and budget policy stay in Mojo.
use super::{InPlaceRecoveryAction, decode_in_place_recovery_action};
use crate::{MojoError, launch::views};

/// Observed native fields after the host verifies home, thread and workspace identity.
/// Numeric options retain Serde's signed-integer type check, including overflow rejection.
#[derive(Clone, Copy, Debug)]
pub struct NativeRecoveryWireInput<'a> {
    pub thread_status: Option<&'a str>,
    pub expected_goal: bool,
    pub goal_present: bool,
    pub goal_status: Option<&'a str>,
    pub token_budget_present: bool,
    pub token_budget_is_null: bool,
    pub token_budget: Option<i64>,
    pub tokens_used: Option<i64>,
}

unsafe extern "C" {
    fn prodex_launch_native_recovery_wire_v1(
        abi_version: i64,
        statuses_address: u64,
        statuses_count: i64,
        flags: i64,
        token_budget: i64,
        tokens_used: i64,
    ) -> i64;
}

/// Classify native statuses and budgets, then use the existing in-place recovery policy.
pub fn in_place_recovery_action_from_wire(
    input: NativeRecoveryWireInput<'_>,
) -> Result<InPlaceRecoveryAction, MojoError> {
    let statuses = views(&[input.thread_status, input.goal_status])?;
    let flags = i64::from(input.goal_present)
        | (i64::from(input.token_budget_present) << 1)
        | (i64::from(input.token_budget_is_null) << 2)
        | (i64::from(input.token_budget.is_some()) << 3)
        | (i64::from(input.tokens_used.is_some()) << 4)
        | (i64::from(input.expected_goal) << 5);
    // SAFETY: the two existing ABI string views borrow live UTF-8 strings for this call.
    decode_in_place_recovery_action(unsafe {
        prodex_launch_native_recovery_wire_v1(
            1,
            statuses.as_ptr() as u64,
            statuses.len() as i64,
            flags,
            input.token_budget.unwrap_or_default(),
            input.tokens_used.unwrap_or_default(),
        )
    })
}

#[cfg(test)]
#[path = "native_wire_tests.rs"]
mod tests;
