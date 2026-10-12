//! Mojo-owned SSE tap-state decisions for runtime response forwarding.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MODE_RESPONSE_ID: i64 = 0;
const MODE_TOKEN_USAGE: i64 = 1;

const RESPONSE_ID_RECORD: i64 = 1;
const RESPONSE_ID_RECORD_WITH_TURN_STATE: i64 = 2;
const RESPONSE_ID_BACKFILL_TURN_STATE: i64 = 4;

const TOKEN_USAGE_NONE: i64 = 0;
const TOKEN_USAGE_LOG: i64 = 1;
const TOKEN_USAGE_LOG_WITH_GENERATION: i64 = 2;

unsafe extern "C" {
    fn prodex_runtime_response_forwarding_tap_state_plan_v1(
        abi_version: i64,
        mode: i64,
        already_remembered: i64,
        turn_state_present: i64,
        remembered_count: u64,
        bound_count: u64,
        token_usage_present: i64,
        duplicate_token_usage: i64,
        loggable_usage: i64,
        generation_present: i64,
    ) -> i64;
}

/// Facts needed to plan one response-ID update or turn-state backfill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseForwardingResponseIdInput {
    pub already_remembered: bool,
    pub turn_state_present: bool,
    pub remembered_count: u64,
    pub bound_count: u64,
}

/// Host effects selected for one response-ID update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseForwardingResponseIdPlan {
    pub record: bool,
    pub record_with_turn_state: bool,
    pub backfill_turn_state: bool,
}

/// Facts needed to plan one token-usage logging effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseForwardingTokenUsageInput {
    pub token_usage_present: bool,
    pub duplicate_token_usage: bool,
    pub loggable_usage: bool,
    pub generation_present: bool,
}

/// Host effect selected for one token-usage observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponseForwardingTokenUsagePlan {
    None,
    Log,
    LogWithGeneration,
}

fn call(
    mode: i64,
    already_remembered: bool,
    turn_state_present: bool,
    remembered_count: u64,
    bound_count: u64,
    input: RuntimeResponseForwardingTokenUsageInput,
) -> Result<i64, MojoError> {
    let result = unsafe {
        prodex_runtime_response_forwarding_tap_state_plan_v1(
            ABI_VERSION,
            mode,
            i64::from(already_remembered),
            i64::from(turn_state_present),
            remembered_count,
            bound_count,
            i64::from(input.token_usage_present),
            i64::from(input.duplicate_token_usage),
            i64::from(input.loggable_usage),
            i64::from(input.generation_present),
        )
    };
    match result {
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        value => Ok(value),
    }
}

/// Select response-ID recording and turn-state backfill effects.
pub fn response_id_plan(
    input: RuntimeResponseForwardingResponseIdInput,
) -> Result<RuntimeResponseForwardingResponseIdPlan, MojoError> {
    let result = call(
        MODE_RESPONSE_ID,
        input.already_remembered,
        input.turn_state_present,
        input.remembered_count,
        input.bound_count,
        RuntimeResponseForwardingTokenUsageInput {
            token_usage_present: false,
            duplicate_token_usage: false,
            loggable_usage: false,
            generation_present: false,
        },
    )?;
    if !(0..=7).contains(&result) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RuntimeResponseForwardingResponseIdPlan {
        record: result & RESPONSE_ID_RECORD != 0,
        record_with_turn_state: result & RESPONSE_ID_RECORD_WITH_TURN_STATE != 0,
        backfill_turn_state: result & RESPONSE_ID_BACKFILL_TURN_STATE != 0,
    })
}

/// Select the deduplicated token-usage logging effect.
pub fn token_usage_plan(
    input: RuntimeResponseForwardingTokenUsageInput,
) -> Result<RuntimeResponseForwardingTokenUsagePlan, MojoError> {
    let result = call(MODE_TOKEN_USAGE, false, false, 0, 0, input)?;
    match result {
        TOKEN_USAGE_NONE => Ok(RuntimeResponseForwardingTokenUsagePlan::None),
        TOKEN_USAGE_LOG => Ok(RuntimeResponseForwardingTokenUsagePlan::Log),
        TOKEN_USAGE_LOG_WITH_GENERATION => {
            Ok(RuntimeResponseForwardingTokenUsagePlan::LogWithGeneration)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_id_input() -> RuntimeResponseForwardingResponseIdInput {
        RuntimeResponseForwardingResponseIdInput {
            already_remembered: false,
            turn_state_present: true,
            remembered_count: 2,
            bound_count: 1,
        }
    }

    fn token_usage_input() -> RuntimeResponseForwardingTokenUsageInput {
        RuntimeResponseForwardingTokenUsageInput {
            token_usage_present: true,
            duplicate_token_usage: false,
            loggable_usage: true,
            generation_present: true,
        }
    }

    #[test]
    fn response_id_plan_preserves_record_and_backfill_precedence() {
        assert_eq!(
            response_id_plan(response_id_input()).unwrap(),
            RuntimeResponseForwardingResponseIdPlan {
                record: true,
                record_with_turn_state: true,
                backfill_turn_state: true,
            }
        );
        let mut bound = response_id_input();
        bound.bound_count = 2;
        assert!(!response_id_plan(bound).unwrap().backfill_turn_state);
        let mut existing = response_id_input();
        existing.already_remembered = true;
        assert!(!response_id_plan(existing).unwrap().record);
    }

    #[test]
    fn token_usage_plan_deduplicates_and_preserves_generation_detail() {
        assert_eq!(
            token_usage_plan(token_usage_input()).unwrap(),
            RuntimeResponseForwardingTokenUsagePlan::LogWithGeneration
        );
        let mut plain = token_usage_input();
        plain.generation_present = false;
        assert_eq!(
            token_usage_plan(plain).unwrap(),
            RuntimeResponseForwardingTokenUsagePlan::Log
        );
        let mut duplicate = token_usage_input();
        duplicate.duplicate_token_usage = true;
        assert_eq!(
            token_usage_plan(duplicate).unwrap(),
            RuntimeResponseForwardingTokenUsagePlan::None
        );
    }

    #[test]
    fn tap_state_plans_change_for_each_semantic_fact() {
        let base = response_id_input();
        let mutators: &[fn(&mut RuntimeResponseForwardingResponseIdInput)] = &[
            |input| input.already_remembered = true,
            |input| input.turn_state_present = false,
            |input| input.remembered_count = 1,
            |input| input.bound_count = 2,
        ];
        let expected = response_id_plan(base).unwrap();
        for mutate in mutators {
            let mut candidate = base;
            mutate(&mut candidate);
            assert_ne!(response_id_plan(candidate).unwrap(), expected);
        }

        let base = token_usage_input();
        let mutators: &[fn(&mut RuntimeResponseForwardingTokenUsageInput)] = &[
            |input| input.token_usage_present = false,
            |input| input.duplicate_token_usage = true,
            |input| input.loggable_usage = false,
            |input| input.generation_present = false,
        ];
        let expected = token_usage_plan(base).unwrap();
        for mutate in mutators {
            let mut candidate = base;
            mutate(&mut candidate);
            assert_ne!(token_usage_plan(candidate).unwrap(), expected);
        }
    }

    #[test]
    fn tap_state_abi_rejects_bad_version_and_counts() {
        assert_eq!(
            unsafe {
                prodex_runtime_response_forwarding_tap_state_plan_v1(
                    ABI_VERSION + 1,
                    MODE_RESPONSE_ID,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                )
            },
            -4
        );
        assert_eq!(
            response_id_plan(RuntimeResponseForwardingResponseIdInput {
                remembered_count: 1,
                bound_count: 2,
                ..response_id_input()
            }),
            Err(MojoError::InvalidInput)
        );
    }
}
