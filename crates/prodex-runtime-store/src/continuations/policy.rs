use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeContinuationBindingKind {
    Response,
    TurnState,
    SessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationCompactionPolicy {
    pub response_binding_limit: usize,
    pub turn_state_binding_limit: usize,
    pub session_id_binding_limit: usize,
    pub response_status_limit: usize,
    pub turn_state_status_limit: usize,
    pub session_id_status_limit: usize,
    pub suspect_grace_seconds: i64,
    pub dead_grace_seconds: i64,
    pub verified_stale_seconds: i64,
    pub suspect_not_found_streak_limit: u32,
    pub confidence_max: u32,
}

impl Default for RuntimeContinuationCompactionPolicy {
    fn default() -> Self {
        Self {
            response_binding_limit: 16_384,
            turn_state_binding_limit: 2_048,
            session_id_binding_limit: 2_048,
            response_status_limit: 16_384,
            turn_state_status_limit: 2_048,
            session_id_status_limit: 2_048,
            suspect_grace_seconds: 120,
            dead_grace_seconds: 900,
            verified_stale_seconds: 1_800,
            suspect_not_found_streak_limit: 2,
            confidence_max: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationStatusPolicy {
    pub touch_persist_interval_seconds: i64,
    pub suspect_grace_seconds: i64,
    pub suspect_not_found_streak_limit: u32,
    pub confidence_max: u32,
    pub verified_confidence_bonus: u32,
    pub touch_confidence_bonus: u32,
    pub suspect_confidence_penalty: u32,
}

pub fn runtime_continuation_store_from_app_state(
    state: &AppState,
) -> RuntimeContinuationStore<ResponseProfileBinding> {
    RuntimeContinuationStore {
        response_profile_bindings: runtime_external_response_profile_bindings(
            &state.response_profile_bindings,
        ),
        session_profile_bindings: state.session_profile_bindings.clone(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: runtime_external_session_id_bindings(&state.session_profile_bindings),
        statuses: RuntimeContinuationStatuses::default(),
    }
}
