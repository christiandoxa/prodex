use super::*;

pub(crate) fn runtime_continuation_store_snapshot(
    runtime: &RuntimeRotationState,
) -> RuntimeContinuationStore {
    RuntimeContinuationStore {
        response_profile_bindings: runtime.state.response_profile_bindings.clone(),
        session_profile_bindings: runtime.state.session_profile_bindings.clone(),
        turn_state_bindings: runtime.turn_state_bindings.clone(),
        session_id_bindings: runtime.session_id_bindings.clone(),
        statuses: runtime.continuation_statuses.clone(),
    }
}
