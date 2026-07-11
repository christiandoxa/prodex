use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{RuntimeRouteKind, runtime_route_kind_label};

use super::{
    RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD, RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE,
    RUNTIME_PROFILE_HEALTH_MAX_SCORE, RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE,
    RUNTIME_PROFILE_SUCCESS_STREAK_MAX, runtime_profile_circuit_open_seconds,
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
    current_score
        .saturating_add(delta)
        .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE)
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
    let next_score = current_score
        .saturating_add(delta)
        .min(RUNTIME_PROFILE_HEALTH_MAX_SCORE);

    if next_score < RUNTIME_PROFILE_CIRCUIT_OPEN_THRESHOLD {
        return RuntimeProfileHealthBumpDecision {
            next_score,
            circuit_reopen_stage: None,
            circuit_open_seconds: None,
        };
    }

    let reopen_stage = if circuit_already_open {
        current_circuit_reopen_stage
            .saturating_add(1)
            .min(RUNTIME_PROFILE_CIRCUIT_REOPEN_MAX_STAGE)
    } else {
        0
    };

    RuntimeProfileHealthBumpDecision {
        next_score,
        circuit_reopen_stage: Some(reopen_stage),
        circuit_open_seconds: Some(runtime_profile_circuit_open_seconds(
            next_score,
            reopen_stage,
        )),
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
    let Some(current_score) = current_score else {
        return RuntimeProfileHealthRecoveryDecision {
            next_score: None,
            next_success_streak: None,
        };
    };

    let next_success_streak = current_success_streak
        .saturating_add(1)
        .min(RUNTIME_PROFILE_SUCCESS_STREAK_MAX);
    let recovery = RUNTIME_PROFILE_HEALTH_SUCCESS_RECOVERY_SCORE
        .saturating_add(next_success_streak.saturating_sub(1).min(1));
    let next_score = current_score.saturating_sub(recovery);

    if next_score == 0 {
        RuntimeProfileHealthRecoveryDecision {
            next_score: None,
            next_success_streak: None,
        }
    } else {
        RuntimeProfileHealthRecoveryDecision {
            next_score: Some(next_score),
            next_success_streak: Some(next_success_streak),
        }
    }
}
