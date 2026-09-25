use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{RuntimeRouteKind, runtime_route_kind_label};

use super::{
    RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD, RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE,
    RUNTIME_PROFILE_HEALTH_MAX_SCORE, RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE,
    RUNTIME_PROFILE_SUCCESS_STREAK_MAX,
};

pub fn runtime_profile_selection_jitter(
    request_sequence: u64,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    request_sequence.hash(&mut hasher);
    profile_name.hash(&mut hasher);
    runtime_route_kind_label(route_kind).hash(&mut hasher);
    hasher.finish()
}

pub fn runtime_profile_bad_pairing_next_score(current_score: u32, delta: u32) -> u32 {
    prodex_mojo_core::runtime::profile_bad_pairing_next_score(
        current_score,
        delta,
        RUNTIME_PROFILE_HEALTH_MAX_SCORE,
    )
    .unwrap_or_else(|error| panic!("Mojo bad-pairing score failed: {error:?}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProfileHealthBumpDecision {
    pub next_score: u32,
    pub circuit_reopen_stage: Option<u32>,
    pub circuit_open_seconds: Option<i64>,
}

pub fn runtime_profile_health_bump_decision(
    current_score: u32,
    delta: u32,
    circuit_already_open: bool,
    current_circuit_reopen_stage: u32,
) -> RuntimeProfileHealthBumpDecision {
    let plan = prodex_mojo_core::runtime::profile_health_bump_plan(
        prodex_mojo_core::runtime::ProfileHealthBumpInput {
            current_score,
            delta,
            max_score: RUNTIME_PROFILE_HEALTH_MAX_SCORE,
            circuit_open_threshold: RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD,
            circuit_already_open,
            current_reopen_stage: current_circuit_reopen_stage,
            max_reopen_stage: RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE,
            circuit_open_seconds: super::RUNTIME_PROFILE_CIRCUIT_OPEN_SECONDS,
            circuit_open_max_seconds: super::RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS,
        },
    )
    .unwrap_or_else(|error| panic!("Mojo profile health bump failed: {error:?}"));
    RuntimeProfileHealthBumpDecision {
        next_score: plan.next_score,
        circuit_reopen_stage: plan.circuit_reopen_stage,
        circuit_open_seconds: plan.circuit_open_seconds,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProfileHealthRecoveryDecision {
    pub next_score: Option<u32>,
    pub next_success_streak: Option<u32>,
}

pub fn runtime_profile_health_recovery_decision(
    current_score: Option<u32>,
    current_success_streak: u32,
) -> RuntimeProfileHealthRecoveryDecision {
    let plan = prodex_mojo_core::runtime::profile_health_recovery_plan(
        current_score,
        current_success_streak,
        RUNTIME_PROFILE_SUCCESS_STREAK_MAX,
        RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE,
    )
    .unwrap_or_else(|error| panic!("Mojo profile health recovery failed: {error:?}"));
    RuntimeProfileHealthRecoveryDecision {
        next_score: plan.next_score,
        next_success_streak: plan.next_success_streak,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_policy_saturates_at_score_and_stage_limits() {
        let max_score = RUNTIME_PROFILE_HEALTH_MAX_SCORE;
        assert_eq!(runtime_profile_bad_pairing_next_score(3, 2), 5);
        assert_eq!(
            runtime_profile_bad_pairing_next_score(max_score - 1, 2),
            max_score
        );

        assert_eq!(
            runtime_profile_health_bump_decision(max_score - 1, u32::MAX, true, u32::MAX),
            RuntimeProfileHealthBumpDecision {
                next_score: max_score,
                circuit_reopen_stage: Some(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE),
                circuit_open_seconds: Some(crate::RUNTIME_PROFILE_CIRCUIT_OPEN_MAX_SECONDS),
            },
        );
        assert_eq!(
            runtime_profile_health_recovery_decision(Some(max_score), u32::MAX),
            RuntimeProfileHealthRecoveryDecision {
                next_score: Some(13),
                next_success_streak: Some(RUNTIME_PROFILE_SUCCESS_STREAK_MAX),
            },
        );
    }
}
