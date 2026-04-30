pub fn runtime_websocket_should_promote_committed_profile(
    previous_response_id: Option<&str>,
    bound_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    compact_followup_profile: Option<&(String, &'static str)>,
    _request_session_id_header_present: bool,
    bound_session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_none()
        && bound_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && compact_followup_profile.is_none()
        && bound_session_profile.is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWebsocketMessageLoopAction {
    Continue,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWebsocketDirectCurrentFallbackReason {
    PrecommitBudgetExhausted,
    CandidateExhausted,
}

impl RuntimeWebsocketDirectCurrentFallbackReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrecommitBudgetExhausted => "precommit_budget_exhausted",
            Self::CandidateExhausted => "candidate_exhausted",
        }
    }

    pub fn reset_previous_response_retry_index_on_local_block(self) -> bool {
        matches!(self, Self::PrecommitBudgetExhausted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promote_committed_profile_only_without_existing_affinity() {
        assert!(runtime_websocket_should_promote_committed_profile(
            None, None, None, None, None, false, None
        ));
        assert!(!runtime_websocket_should_promote_committed_profile(
            Some("resp_1"),
            None,
            None,
            None,
            None,
            false,
            None,
        ));
        assert!(!runtime_websocket_should_promote_committed_profile(
            None,
            None,
            Some("turn-state"),
            None,
            None,
            false,
            None,
        ));
        assert!(!runtime_websocket_should_promote_committed_profile(
            None,
            None,
            None,
            None,
            None,
            false,
            Some("alpha"),
        ));
    }

    #[test]
    fn direct_current_fallback_reason_labels_match_runtime_logs() {
        assert_eq!(
            RuntimeWebsocketDirectCurrentFallbackReason::PrecommitBudgetExhausted.as_str(),
            "precommit_budget_exhausted"
        );
        assert!(
            RuntimeWebsocketDirectCurrentFallbackReason::PrecommitBudgetExhausted
                .reset_previous_response_retry_index_on_local_block()
        );
        assert!(
            !RuntimeWebsocketDirectCurrentFallbackReason::CandidateExhausted
                .reset_previous_response_retry_index_on_local_block()
        );
    }
}
