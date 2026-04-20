use super::*;

pub(crate) fn compact_runtime_continuation_state_in_place(runtime: &mut RuntimeRotationState) {
    let continuations = RuntimeContinuationStore {
        response_profile_bindings: std::mem::take(&mut runtime.state.response_profile_bindings),
        session_profile_bindings: std::mem::take(&mut runtime.state.session_profile_bindings),
        turn_state_bindings: std::mem::take(&mut runtime.turn_state_bindings),
        session_id_bindings: std::mem::take(&mut runtime.session_id_bindings),
        statuses: std::mem::take(&mut runtime.continuation_statuses),
    };
    let compacted = compact_runtime_continuation_store(continuations, &runtime.state.profiles);
    runtime.state.response_profile_bindings = compacted.response_profile_bindings;
    runtime.state.session_profile_bindings = compacted.session_profile_bindings;
    runtime.turn_state_bindings = compacted.turn_state_bindings;
    runtime.session_id_bindings = compacted.session_id_bindings;
    runtime.continuation_statuses = compacted.statuses;
}

pub(crate) fn runtime_continuation_store_snapshot(
    runtime: &RuntimeRotationState,
) -> RuntimeContinuationStore {
    compact_runtime_continuation_store(
        RuntimeContinuationStore {
            response_profile_bindings: runtime.state.response_profile_bindings.clone(),
            session_profile_bindings: runtime.state.session_profile_bindings.clone(),
            turn_state_bindings: runtime.turn_state_bindings.clone(),
            session_id_bindings: runtime.session_id_bindings.clone(),
            statuses: runtime.continuation_statuses.clone(),
        },
        &runtime.state.profiles,
    )
}
