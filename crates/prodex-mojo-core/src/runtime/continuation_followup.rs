use super::continuation_status_transition;

const COMPACT_FOLLOWUP_PLAN: i64 = 24;

/// The compact continuation decision returned by the Mojo policy kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeContinuationFollowupAction {
    None,
    Owner,
    Conflict,
}

/// Rust-resolved continuation facts passed to the compact follow-up policy.
///
/// Maps, profile validity, authentication health, and timestamps stay in Rust;
/// Mojo owns the precedence and touch decision for these facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationFollowupInput {
    pub status_present: bool,
    pub state: i64,
    pub confidence: u32,
    pub last_touched_at: Option<i64>,
    pub last_verified_at: Option<i64>,
    pub last_not_found_at: Option<i64>,
    pub not_found_streak: u32,
    pub failure_count: u32,
    pub binding_present: bool,
    pub binding_conflict: bool,
    pub binding_bound_at: i64,
    pub now: i64,
    pub verified_stale_seconds: i64,
    pub suspect_grace_seconds: i64,
    pub suspect_not_found_streak_limit: u32,
    pub touch_persist_interval_seconds: i64,
}

/// Pure compact follow-up decisions produced by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationFollowupPlan {
    pub action: RuntimeContinuationFollowupAction,
    pub status_stale: bool,
    pub recently_suspect: bool,
    pub dead_shadowed_by_binding: bool,
    pub dead: bool,
    pub persist_touch: bool,
}

fn optional_time(value: Option<i64>) -> (i64, i64) {
    value.map_or((0, 0), |value| (1, value))
}

fn boolean(value: i64) -> Result<bool, crate::MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

/// Runs the versioned compact continuation policy without a Rust fallback.
pub fn runtime_continuation_compact_followup_plan(
    input: RuntimeContinuationFollowupInput,
) -> Result<RuntimeContinuationFollowupPlan, crate::MojoError> {
    let (touched_present, touched_at) = optional_time(input.last_touched_at);
    let (verified_present, verified_at) = optional_time(input.last_verified_at);
    let (not_found_present, not_found_at) = optional_time(input.last_not_found_at);
    let fields = [
        input.state,
        i64::from(input.confidence),
        touched_present,
        touched_at,
        verified_present,
        verified_at,
        0,
        not_found_present,
        not_found_at,
        i64::from(input.not_found_streak),
        0,
        i64::from(input.failure_count),
        i64::from(input.status_present),
        i64::from(input.binding_present),
        i64::from(input.binding_conflict),
        input.binding_bound_at,
        input.now,
        input.verified_stale_seconds,
        input.suspect_grace_seconds,
        i64::from(input.suspect_not_found_streak_limit),
        input.touch_persist_interval_seconds,
    ];
    let output = continuation_status_transition::<6>(COMPACT_FOLLOWUP_PLAN, &fields)?;
    let action = match output[0] {
        0 => RuntimeContinuationFollowupAction::None,
        1 => RuntimeContinuationFollowupAction::Owner,
        2 => RuntimeContinuationFollowupAction::Conflict,
        _ => return Err(crate::MojoError::InvalidOutput),
    };
    Ok(RuntimeContinuationFollowupPlan {
        action,
        status_stale: boolean(output[1])?,
        recently_suspect: boolean(output[2])?,
        dead_shadowed_by_binding: boolean(output[3])?,
        dead: boolean(output[4])?,
        persist_touch: boolean(output[5])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000;
    const STALE: i64 = 60;
    const GRACE: i64 = 30;
    const LIMIT: u32 = 2;
    const TOUCH: i64 = 10;

    fn input() -> RuntimeContinuationFollowupInput {
        RuntimeContinuationFollowupInput {
            status_present: false,
            state: 0,
            confidence: 0,
            last_touched_at: None,
            last_verified_at: None,
            last_not_found_at: None,
            not_found_streak: 0,
            failure_count: 0,
            binding_present: false,
            binding_conflict: false,
            binding_bound_at: NOW,
            now: NOW,
            verified_stale_seconds: STALE,
            suspect_grace_seconds: GRACE,
            suspect_not_found_streak_limit: LIMIT,
            touch_persist_interval_seconds: TOUCH,
        }
    }

    #[test]
    fn compact_followup_golden_precedence_keeps_live_owner_over_tombstones() {
        let mut cases = Vec::new();
        let mut unbound = input();
        cases.push((unbound, RuntimeContinuationFollowupAction::None));

        unbound.status_present = true;
        unbound.state = 3;
        unbound.last_not_found_at = Some(NOW);
        unbound.not_found_streak = LIMIT;
        cases.push((unbound, RuntimeContinuationFollowupAction::None));

        let mut terminal_suspect = input();
        terminal_suspect.status_present = true;
        terminal_suspect.state = 2;
        terminal_suspect.not_found_streak = LIMIT;
        terminal_suspect.last_not_found_at = Some(NOW);
        terminal_suspect.binding_present = true;
        terminal_suspect.binding_bound_at = NOW + 1;
        cases.push((terminal_suspect, RuntimeContinuationFollowupAction::None));

        let mut live = unbound;
        live.binding_present = true;
        live.binding_bound_at = NOW + 1;
        cases.push((live, RuntimeContinuationFollowupAction::Owner));

        let mut conflict = live;
        conflict.binding_conflict = true;
        cases.push((conflict, RuntimeContinuationFollowupAction::Conflict));

        for (case, action) in cases {
            assert_eq!(
                runtime_continuation_compact_followup_plan(case)
                    .expect("golden compact policy case")
                    .action,
                action
            );
        }
    }

    #[test]
    fn compact_followup_boundaries_preserve_stale_suspect_and_touch_rules() {
        let mut stale = input();
        stale.status_present = true;
        stale.state = 1;
        stale.last_verified_at = Some(NOW - STALE);
        let plan = runtime_continuation_compact_followup_plan(stale).expect("stale boundary");
        assert!(plan.status_stale);

        let mut suspect = input();
        suspect.status_present = true;
        suspect.state = 2;
        suspect.last_not_found_at = Some(NOW - GRACE);
        suspect.binding_present = true;
        assert!(
            !runtime_continuation_compact_followup_plan(suspect)
                .expect("suspect boundary")
                .recently_suspect
        );
        suspect.last_not_found_at = Some(NOW - GRACE + 1);
        assert!(
            runtime_continuation_compact_followup_plan(suspect)
                .expect("recent suspect boundary")
                .recently_suspect
        );

        let mut touch = input();
        touch.status_present = true;
        touch.binding_present = true;
        touch.binding_bound_at = NOW - TOUCH;
        touch.last_touched_at = Some(NOW);
        assert!(
            !runtime_continuation_compact_followup_plan(touch)
                .expect("touch equality boundary")
                .persist_touch
        );
        touch.binding_bound_at = NOW - TOUCH - 1;
        assert!(
            runtime_continuation_compact_followup_plan(touch)
                .expect("touch interval boundary")
                .persist_touch
        );
    }

    #[test]
    fn compact_followup_stateful_recovery_releases_dead_tombstone_only_for_newer_binding() {
        let mut state = input();
        state.status_present = true;
        state.state = 3;
        state.last_not_found_at = Some(NOW);
        state.not_found_streak = LIMIT;
        state.binding_present = true;
        state.binding_bound_at = NOW;
        assert_eq!(
            runtime_continuation_compact_followup_plan(state)
                .expect("dead owner at equal timestamp")
                .action,
            RuntimeContinuationFollowupAction::None
        );
        state.binding_bound_at = NOW + 1;
        let plan = runtime_continuation_compact_followup_plan(state)
            .expect("newer owner should shadow dead tombstone");
        assert_eq!(plan.action, RuntimeContinuationFollowupAction::Owner);
        assert!(plan.dead_shadowed_by_binding);
    }

    #[test]
    fn compact_followup_timestamp_arithmetic_saturates_at_i64_edges() {
        let mut future = input();
        future.status_present = true;
        future.state = 1;
        future.last_verified_at = Some(i64::MAX);
        future.now = i64::MIN;
        assert!(
            !runtime_continuation_compact_followup_plan(future)
                .expect("future timestamp subtraction saturates")
                .status_stale
        );

        let mut old_binding = input();
        old_binding.binding_present = true;
        old_binding.binding_bound_at = i64::MIN;
        old_binding.now = i64::MAX;
        assert!(
            runtime_continuation_compact_followup_plan(old_binding)
                .expect("touch interval subtraction saturates")
                .persist_touch
        );
    }

    #[test]
    fn compact_followup_rejects_malformed_abi_facts() {
        assert_eq!(
            continuation_status_transition::<5>(COMPACT_FOLLOWUP_PLAN, &[0]),
            Err(crate::MojoError::InvalidInput)
        );
        let mut malformed = input();
        malformed.state = 99;
        assert_eq!(
            runtime_continuation_compact_followup_plan(malformed),
            Err(crate::MojoError::InvalidInput)
        );
        malformed = input();
        malformed.verified_stale_seconds = -1;
        assert_eq!(
            runtime_continuation_compact_followup_plan(malformed),
            Err(crate::MojoError::InvalidInput)
        );
    }
}
