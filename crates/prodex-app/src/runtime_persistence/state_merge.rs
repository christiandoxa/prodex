use super::*;

pub(crate) use prodex_state::{merge_last_run_selection, merge_profile_bindings};

pub(crate) fn app_state_compaction_policy() -> prodex_state::AppStateCompactionPolicy {
    prodex_state::AppStateCompactionPolicy {
        last_run_retention_seconds: APP_STATE_LAST_RUN_RETENTION_SECONDS,
        session_binding_retention_seconds: APP_STATE_SESSION_BINDING_RETENTION_SECONDS,
        session_binding_limit: SESSION_ID_PROFILE_BINDING_LIMIT,
    }
}

pub(crate) fn runtime_continuation_compaction_policy()
-> prodex_runtime_store::RuntimeContinuationCompactionPolicy {
    prodex_runtime_store::RuntimeContinuationCompactionPolicy {
        response_binding_limit: RESPONSE_PROFILE_BINDING_LIMIT,
        turn_state_binding_limit: TURN_STATE_PROFILE_BINDING_LIMIT,
        session_id_binding_limit: SESSION_ID_PROFILE_BINDING_LIMIT,
        response_status_limit: RUNTIME_CONTINUATION_RESPONSE_STATUS_LIMIT,
        turn_state_status_limit: RUNTIME_CONTINUATION_TURN_STATE_STATUS_LIMIT,
        session_id_status_limit: RUNTIME_CONTINUATION_SESSION_ID_STATUS_LIMIT,
        suspect_grace_seconds: RUNTIME_CONTINUATION_SUSPECT_GRACE_SECONDS,
        dead_grace_seconds: RUNTIME_CONTINUATION_DEAD_GRACE_SECONDS,
        verified_stale_seconds: RUNTIME_CONTINUATION_VERIFIED_STALE_SECONDS,
        suspect_not_found_streak_limit: RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT,
        confidence_max: RUNTIME_CONTINUATION_CONFIDENCE_MAX,
    }
}

pub(crate) fn compact_app_state(state: AppState, now: i64) -> AppState {
    prodex_state::compact_app_state_with_policy(state, now, app_state_compaction_policy())
}

pub(crate) fn runtime_continuation_status_is_terminal(
    status: &RuntimeContinuationBindingStatus,
) -> bool {
    prodex_runtime_store::runtime_continuation_status_is_terminal(
        status,
        runtime_continuation_compaction_policy(),
    )
}

pub(crate) fn runtime_age_stale_verified_continuation_status(
    statuses: &mut RuntimeContinuationStatuses,
    kind: RuntimeContinuationBindingKind,
    key: &str,
    now: i64,
) -> bool {
    prodex_runtime_store::runtime_age_stale_verified_continuation_status(
        statuses,
        kind,
        key,
        now,
        runtime_continuation_compaction_policy(),
    )
}

pub(crate) fn merge_runtime_state_snapshot(existing: AppState, snapshot: &AppState) -> AppState {
    prodex_runtime_store::merge_runtime_state_snapshot_for_save(
        existing,
        snapshot,
        Local::now().timestamp(),
        app_state_compaction_policy(),
    )
}
