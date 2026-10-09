use super::continuation_status_transition;

const PREVIOUS_RESPONSE_OWNER_PLAN: i64 = 25;
const PREVIOUS_RESPONSE_CANDIDATE_PLAN: i64 = 26;
const BINDING_SOURCE_PLAN: i64 = 27;

/// Rust-observed facts used to classify a previous-response binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePreviousResponseOwnerInput {
    pub id_present: bool,
    pub id_valid: bool,
    pub owner_kind: RuntimeContinuationOwnerKind,
    pub excluded: bool,
    pub auth_failure: bool,
    pub negative_cache: bool,
    pub binding_present: bool,
    pub identity_matches: bool,
}

/// Binding resolution state already materialized by Rust-owned state maps.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeContinuationOwnerKind {
    Unbound,
    Owned,
    Unavailable,
    Conflict,
}

/// Previous-response owner classification selected by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePreviousResponseOwnerAction {
    Unbound,
    Usable,
    Unusable,
    Conflict,
}

/// Classify a previous-response owner without recomputing precedence in Rust.
pub fn runtime_previous_response_owner_plan(
    input: RuntimePreviousResponseOwnerInput,
) -> Result<RuntimePreviousResponseOwnerAction, crate::MojoError> {
    let output = continuation_status_transition::<1>(
        PREVIOUS_RESPONSE_OWNER_PLAN,
        &[
            i64::from(input.id_present),
            i64::from(input.id_valid),
            input.owner_kind as i64,
            i64::from(input.excluded),
            i64::from(input.auth_failure),
            i64::from(input.negative_cache),
            i64::from(input.binding_present),
            i64::from(input.identity_matches),
        ],
    )?;
    match output[0] {
        0 => Ok(RuntimePreviousResponseOwnerAction::Unbound),
        1 => Ok(RuntimePreviousResponseOwnerAction::Usable),
        2 => Ok(RuntimePreviousResponseOwnerAction::Unusable),
        3 => Ok(RuntimePreviousResponseOwnerAction::Conflict),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

/// Rust-observed candidate facts used for previous-response discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePreviousResponseCandidateInput {
    pub negative_cache: bool,
    pub auth_failure: bool,
    pub quota_exhausted: bool,
    pub quota_guard: bool,
    pub cached_auth_present: bool,
    pub cached_auth_compatible: bool,
    pub allow_disk_fallback: bool,
}

/// Candidate action selected by the previous-response discovery kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePreviousResponseCandidateAction {
    RejectNegativeCache,
    RejectAuth,
    RejectQuota,
    SelectCached,
    DiskFallback,
    Skip,
}

/// Select the bounded previous-response discovery path without a Rust fallback.
pub fn runtime_previous_response_candidate_plan(
    input: RuntimePreviousResponseCandidateInput,
) -> Result<RuntimePreviousResponseCandidateAction, crate::MojoError> {
    let output = continuation_status_transition::<1>(
        PREVIOUS_RESPONSE_CANDIDATE_PLAN,
        &[
            i64::from(input.negative_cache),
            i64::from(input.auth_failure),
            i64::from(input.quota_exhausted),
            i64::from(input.quota_guard),
            i64::from(input.cached_auth_present),
            i64::from(input.cached_auth_compatible),
            i64::from(input.allow_disk_fallback),
        ],
    )?;
    match output[0] {
        0 => Ok(RuntimePreviousResponseCandidateAction::RejectNegativeCache),
        1 => Ok(RuntimePreviousResponseCandidateAction::RejectAuth),
        2 => Ok(RuntimePreviousResponseCandidateAction::RejectQuota),
        3 => Ok(RuntimePreviousResponseCandidateAction::SelectCached),
        4 => Ok(RuntimePreviousResponseCandidateAction::DiskFallback),
        5 => Ok(RuntimePreviousResponseCandidateAction::Skip),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

/// Facts for compact follow-up owner/source precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeContinuationBindingSourceInput {
    pub turn_state_present: bool,
    pub session_id_present: bool,
    pub owner_kind: RuntimeContinuationOwnerKind,
}

/// Compact follow-up binding source selected by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeContinuationBindingSource {
    None,
    TurnState,
    SessionId,
    ConflictTurnState,
    ConflictSessionId,
}

/// Resolve compact turn-state/session source precedence without a Rust branch table.
pub fn runtime_continuation_binding_source_plan(
    input: RuntimeContinuationBindingSourceInput,
) -> Result<RuntimeContinuationBindingSource, crate::MojoError> {
    let output = continuation_status_transition::<2>(
        BINDING_SOURCE_PLAN,
        &[
            i64::from(input.turn_state_present),
            i64::from(input.session_id_present),
            input.owner_kind as i64,
        ],
    )?;
    match (output[0], output[1]) {
        (0, 0) => Ok(RuntimeContinuationBindingSource::None),
        (1, 1) => Ok(RuntimeContinuationBindingSource::TurnState),
        (1, 2) => Ok(RuntimeContinuationBindingSource::SessionId),
        (2, 1) => Ok(RuntimeContinuationBindingSource::ConflictTurnState),
        (2, 2) => Ok(RuntimeContinuationBindingSource::ConflictSessionId),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(owner_kind: RuntimeContinuationOwnerKind) -> RuntimePreviousResponseOwnerInput {
        RuntimePreviousResponseOwnerInput {
            id_present: true,
            id_valid: true,
            owner_kind,
            excluded: false,
            auth_failure: false,
            negative_cache: false,
            binding_present: true,
            identity_matches: true,
        }
    }

    #[test]
    fn owner_plan_fails_closed_for_missing_invalid_and_conflicting_bindings() {
        let mut missing = owner(RuntimeContinuationOwnerKind::Unbound);
        missing.id_present = false;
        assert_eq!(
            runtime_previous_response_owner_plan(missing).unwrap(),
            RuntimePreviousResponseOwnerAction::Unbound
        );

        let mut invalid = owner(RuntimeContinuationOwnerKind::Owned);
        invalid.id_valid = false;
        assert_eq!(
            runtime_previous_response_owner_plan(invalid).unwrap(),
            RuntimePreviousResponseOwnerAction::Unusable
        );

        for owner_kind in [
            RuntimeContinuationOwnerKind::Unavailable,
            RuntimeContinuationOwnerKind::Conflict,
        ] {
            let action = runtime_previous_response_owner_plan(owner(owner_kind)).unwrap();
            assert_eq!(
                action,
                if owner_kind == RuntimeContinuationOwnerKind::Conflict {
                    RuntimePreviousResponseOwnerAction::Conflict
                } else {
                    RuntimePreviousResponseOwnerAction::Unusable
                }
            );
        }
    }

    #[test]
    fn owner_plan_precedence_rejects_untrusted_owner_facts() {
        let mutators: &[fn(&mut RuntimePreviousResponseOwnerInput)] = &[
            |input: &mut RuntimePreviousResponseOwnerInput| input.excluded = true,
            |input: &mut RuntimePreviousResponseOwnerInput| input.auth_failure = true,
            |input: &mut RuntimePreviousResponseOwnerInput| input.negative_cache = true,
            |input: &mut RuntimePreviousResponseOwnerInput| input.binding_present = false,
            |input: &mut RuntimePreviousResponseOwnerInput| input.identity_matches = false,
        ];
        for mutate in mutators {
            let mut input = owner(RuntimeContinuationOwnerKind::Owned);
            mutate(&mut input);
            assert_eq!(
                runtime_previous_response_owner_plan(input).unwrap(),
                RuntimePreviousResponseOwnerAction::Unusable
            );
        }
    }

    #[test]
    fn candidate_plan_precedence_prefers_safety_rejections_then_cache() {
        let mut input = RuntimePreviousResponseCandidateInput {
            negative_cache: false,
            auth_failure: false,
            quota_exhausted: false,
            quota_guard: false,
            cached_auth_present: false,
            cached_auth_compatible: false,
            allow_disk_fallback: true,
        };
        assert_eq!(
            runtime_previous_response_candidate_plan(input).unwrap(),
            RuntimePreviousResponseCandidateAction::DiskFallback
        );
        input.cached_auth_present = true;
        input.cached_auth_compatible = true;
        assert_eq!(
            runtime_previous_response_candidate_plan(input).unwrap(),
            RuntimePreviousResponseCandidateAction::SelectCached
        );
        input.cached_auth_compatible = false;
        assert_eq!(
            runtime_previous_response_candidate_plan(input).unwrap(),
            RuntimePreviousResponseCandidateAction::Skip
        );
        input.quota_guard = true;
        assert_eq!(
            runtime_previous_response_candidate_plan(input).unwrap(),
            RuntimePreviousResponseCandidateAction::RejectQuota
        );
        input.auth_failure = true;
        assert_eq!(
            runtime_previous_response_candidate_plan(input).unwrap(),
            RuntimePreviousResponseCandidateAction::RejectAuth
        );
        input.negative_cache = true;
        assert_eq!(
            runtime_previous_response_candidate_plan(input).unwrap(),
            RuntimePreviousResponseCandidateAction::RejectNegativeCache
        );
    }

    #[test]
    fn binding_source_plan_keeps_turn_state_ahead_of_session_and_conflict_fail_closed() {
        let owner = RuntimeContinuationOwnerKind::Owned;
        assert_eq!(
            runtime_continuation_binding_source_plan(RuntimeContinuationBindingSourceInput {
                turn_state_present: true,
                session_id_present: true,
                owner_kind: owner,
            })
            .unwrap(),
            RuntimeContinuationBindingSource::TurnState
        );
        assert_eq!(
            runtime_continuation_binding_source_plan(RuntimeContinuationBindingSourceInput {
                turn_state_present: false,
                session_id_present: true,
                owner_kind: owner,
            })
            .unwrap(),
            RuntimeContinuationBindingSource::SessionId
        );
        assert_eq!(
            runtime_continuation_binding_source_plan(RuntimeContinuationBindingSourceInput {
                turn_state_present: true,
                session_id_present: true,
                owner_kind: RuntimeContinuationOwnerKind::Conflict,
            })
            .unwrap(),
            RuntimeContinuationBindingSource::ConflictTurnState
        );
    }
}
