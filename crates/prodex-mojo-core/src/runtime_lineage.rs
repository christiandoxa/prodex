use crate::MojoError;

const ABI_VERSION: i64 = 1;
const KEY_MAX_BYTES: usize = 4_096;
const CLASS_COMPONENT: i64 = 0;
const CLASS_KEY: i64 = 1;
const CLASS_RESPONSE_TURN_KEY: i64 = 2;
const CLASS_COMPACT_SESSION_KEY: i64 = 3;
const BUILD_COMPACT_SESSION: i64 = 0;
const BUILD_COMPACT_TURN_STATE: i64 = 1;
const BUILD_RESPONSE_TURN_STATE: i64 = 2;

unsafe extern "C" {
    fn prodex_runtime_lineage_classify_v1(
        abi_version: i64,
        kind: i64,
        address: u64,
        length: i64,
    ) -> i64;
    fn prodex_runtime_lineage_identity_fallback_v1(
        abi_version: i64,
        response_address: u64,
        response_length: i64,
        turn_state_address: u64,
        turn_state_length: i64,
        session_address: u64,
        session_length: i64,
    ) -> i64;
    fn prodex_runtime_lineage_build_v1(
        abi_version: i64,
        kind: i64,
        first_address: u64,
        first_length: i64,
        second_address: u64,
        second_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_lineage_candidate_plan_v1(
        abi_version: i64,
        profile_valid: i64,
        conflict_sentinel: i64,
        profile_available: i64,
        binding_identity_present: i64,
        existing_identity_present: i64,
        binding_identity_matches: i64,
    ) -> i64;
    fn prodex_runtime_lineage_resolution_plan_v1(
        abi_version: i64,
        conflict: i64,
        owner_count: i64,
        unavailable_count: i64,
    ) -> i64;
    fn prodex_runtime_lineage_lookup_affinity_v1(
        abi_version: i64,
        turn_state_present: i64,
        bound_present: i64,
        fallback_present: i64,
    ) -> i64;
    fn prodex_runtime_lineage_owner_lookup_v1(
        abi_version: i64,
        owner_kind: i64,
        expected_identity_present: i64,
        identity_matches: i64,
    ) -> i64;
    fn prodex_runtime_lineage_release_plan_v1(
        abi_version: i64,
        previous_response_present: i64,
        previous_response_matches: i64,
        turn_state_present: i64,
        turn_state_matches: i64,
        session_present: i64,
        session_matches: i64,
        compact_session_matches: i64,
    ) -> i64;
    fn prodex_runtime_lineage_parts_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
}

#[path = "runtime_lineage/binding_candidate.rs"]
mod binding_candidate;
pub use binding_candidate::{
    RuntimeLineageDispatchBindingCandidate, RuntimeLineageDispatchBindingDecision,
    RuntimeLineageLocalRewriteCandidate, RuntimeLineageProviderBindingIdentity,
    dispatch_binding_candidate_decision, local_rewrite_candidate_allowed,
};

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn classify(kind: i64, value: &str) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_lineage_classify_v1(
            ABI_VERSION,
            kind,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn component_valid(value: &str) -> Result<bool, MojoError> {
    classify(CLASS_COMPONENT, value)
}

pub fn key_valid(value: &str) -> Result<bool, MojoError> {
    classify(CLASS_KEY, value)
}

pub fn is_response_turn_key(value: &str) -> Result<bool, MojoError> {
    classify(CLASS_RESPONSE_TURN_KEY, value)
}

pub fn is_compact_session_key(value: &str) -> Result<bool, MojoError> {
    classify(CLASS_COMPACT_SESSION_KEY, value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeLineageIdentityFallback {
    Empty,
    Invalid,
}

fn optional_string_parts(value: Option<&str>) -> Result<(u64, i64), MojoError> {
    match value {
        Some(value) => Ok((value.as_ptr() as usize as u64, signed_len(value)?)),
        None => Ok((0, 0)),
    }
}

pub fn identity_fallback_plan(
    response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<RuntimeLineageIdentityFallback, MojoError> {
    let (response_address, response_length) = optional_string_parts(response_id)?;
    let (turn_state_address, turn_state_length) = optional_string_parts(turn_state)?;
    let (session_address, session_length) = optional_string_parts(session_id)?;
    match unsafe {
        prodex_runtime_lineage_identity_fallback_v1(
            ABI_VERSION,
            response_address,
            response_length,
            turn_state_address,
            turn_state_length,
            session_address,
            session_length,
        )
    } {
        0 => Ok(RuntimeLineageIdentityFallback::Empty),
        1 => Ok(RuntimeLineageIdentityFallback::Invalid),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn build(kind: i64, first: &str, second: Option<&str>) -> Result<String, MojoError> {
    let second = second.unwrap_or_default();
    let mut output = vec![0_u8; KEY_MAX_BYTES];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_runtime_lineage_build_v1(
            ABI_VERSION,
            kind,
            first.as_ptr() as usize as u64,
            signed_len(first)?,
            second.as_ptr() as usize as u64,
            signed_len(second)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(if status == 3 {
            MojoError::Capacity
        } else {
            MojoError::InvalidInput
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

pub fn compact_session_key(value: &str) -> Result<String, MojoError> {
    build(BUILD_COMPACT_SESSION, value, None)
}

pub fn compact_turn_state_key(value: &str) -> Result<String, MojoError> {
    build(BUILD_COMPACT_TURN_STATE, value, None)
}

pub fn response_turn_state_key(response_id: &str, turn_state: &str) -> Result<String, MojoError> {
    build(BUILD_RESPONSE_TURN_STATE, response_id, Some(turn_state))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeLineageCandidateKind {
    Owner,
    Unavailable,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeLineageIdentityAction {
    Keep,
    Set,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeLineageCandidatePlan {
    pub kind: RuntimeLineageCandidateKind,
    pub identity_action: RuntimeLineageIdentityAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeLineageResolutionKind {
    Unbound,
    Owned,
    Unavailable,
    Conflict,
}

pub fn candidate_plan(
    profile_valid: bool,
    conflict_sentinel: bool,
    profile_available: bool,
    binding_identity_present: bool,
    existing_identity_present: bool,
    binding_identity_matches: bool,
) -> Result<RuntimeLineageCandidatePlan, MojoError> {
    let result = unsafe {
        prodex_runtime_lineage_candidate_plan_v1(
            ABI_VERSION,
            i64::from(profile_valid),
            i64::from(conflict_sentinel),
            i64::from(profile_available),
            i64::from(binding_identity_present),
            i64::from(existing_identity_present),
            i64::from(binding_identity_matches),
        )
    };
    if result == -4 {
        return Err(MojoError::AbiMismatch);
    }
    if result < 0 {
        return Err(MojoError::InvalidInput);
    }
    let kind = match result & 0xff {
        1 => RuntimeLineageCandidateKind::Owner,
        2 => RuntimeLineageCandidateKind::Unavailable,
        3 => RuntimeLineageCandidateKind::Conflict,
        _ => return Err(MojoError::InvalidOutput),
    };
    let identity_action = match (result >> 8) & 0xff {
        0 => RuntimeLineageIdentityAction::Keep,
        1 => RuntimeLineageIdentityAction::Set,
        2 => RuntimeLineageIdentityAction::Conflict,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(RuntimeLineageCandidatePlan {
        kind,
        identity_action,
    })
}

pub fn resolution_plan(
    conflict: bool,
    owner_count: usize,
    unavailable_count: usize,
) -> Result<RuntimeLineageResolutionKind, MojoError> {
    let result = unsafe {
        prodex_runtime_lineage_resolution_plan_v1(
            ABI_VERSION,
            i64::from(conflict),
            i64::try_from(owner_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(unavailable_count).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        0 => Ok(RuntimeLineageResolutionKind::Unbound),
        1 => Ok(RuntimeLineageResolutionKind::Owned),
        2 => Ok(RuntimeLineageResolutionKind::Unavailable),
        3 => Ok(RuntimeLineageResolutionKind::Conflict),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeLineageLookupAffinity {
    None,
    Bound,
    Fallback,
    Current,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeLineageOwnerKind {
    Unbound,
    Owned,
    Unavailable,
    Conflict,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeLineageOwnerProfileAction {
    None,
    Owner,
    ConflictSentinel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeLineageOwnerLookupPlan {
    pub owner_kind: RuntimeLineageOwnerKind,
    pub profile_action: RuntimeLineageOwnerProfileAction,
}

pub fn lookup_affinity_plan(
    turn_state_present: bool,
    bound_present: bool,
    fallback_present: bool,
) -> Result<RuntimeLineageLookupAffinity, MojoError> {
    let result = unsafe {
        prodex_runtime_lineage_lookup_affinity_v1(
            ABI_VERSION,
            i64::from(turn_state_present),
            i64::from(bound_present),
            i64::from(fallback_present),
        )
    };
    match result {
        0 => Ok(RuntimeLineageLookupAffinity::None),
        1 => Ok(RuntimeLineageLookupAffinity::Bound),
        2 => Ok(RuntimeLineageLookupAffinity::Fallback),
        3 => Ok(RuntimeLineageLookupAffinity::Current),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn owner_lookup_plan(
    owner_kind: RuntimeLineageOwnerKind,
    expected_identity_present: bool,
    identity_matches: bool,
) -> Result<RuntimeLineageOwnerLookupPlan, MojoError> {
    let owner_tag = match owner_kind {
        RuntimeLineageOwnerKind::Unbound => 0,
        RuntimeLineageOwnerKind::Owned => 1,
        RuntimeLineageOwnerKind::Unavailable => 2,
        RuntimeLineageOwnerKind::Conflict => 3,
    };
    let result = unsafe {
        prodex_runtime_lineage_owner_lookup_v1(
            ABI_VERSION,
            owner_tag,
            i64::from(expected_identity_present),
            i64::from(identity_matches),
        )
    };
    if result < 0 {
        return Err(MojoError::InvalidInput);
    }
    let owner_kind = match result & 0xff {
        0 => RuntimeLineageOwnerKind::Unbound,
        1 => RuntimeLineageOwnerKind::Owned,
        2 => RuntimeLineageOwnerKind::Unavailable,
        3 => RuntimeLineageOwnerKind::Conflict,
        _ => return Err(MojoError::InvalidOutput),
    };
    let profile_action = match (result >> 8) & 0xff {
        0 => RuntimeLineageOwnerProfileAction::None,
        1 => RuntimeLineageOwnerProfileAction::Owner,
        2 => RuntimeLineageOwnerProfileAction::ConflictSentinel,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(RuntimeLineageOwnerLookupPlan {
        owner_kind,
        profile_action,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeLineageReleasePlan {
    pub response: bool,
    pub turn_state: bool,
    pub session: bool,
    pub compact_session: bool,
}

pub fn release_plan(
    previous_response_present: bool,
    previous_response_matches: bool,
    turn_state_present: bool,
    turn_state_matches: bool,
    session_present: bool,
    session_matches: bool,
    compact_session_matches: bool,
) -> Result<RuntimeLineageReleasePlan, crate::MojoError> {
    let mask = unsafe {
        prodex_runtime_lineage_release_plan_v1(
            ABI_VERSION,
            i64::from(previous_response_present),
            i64::from(previous_response_matches),
            i64::from(turn_state_present),
            i64::from(turn_state_matches),
            i64::from(session_present),
            i64::from(session_matches),
            i64::from(compact_session_matches),
        )
    };
    if mask == -4 {
        return Err(crate::MojoError::AbiMismatch);
    }
    if !(0..=15).contains(&mask) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(RuntimeLineageReleasePlan {
        response: mask & 1 != 0,
        turn_state: mask & 2 != 0,
        session: mask & 4 != 0,
        compact_session: mask & 8 != 0,
    })
}

pub fn response_turn_state_parts(
    key: &str,
) -> Result<Option<(usize, usize, usize, usize)>, MojoError> {
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_runtime_lineage_parts_v1(
            ABI_VERSION,
            key.as_ptr() as usize as u64,
            signed_len(key)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    if output[0] < 0 {
        return Ok(None);
    }
    Ok(Some((
        usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?,
        usize::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?,
    )))
}

#[cfg(test)]
mod hard_binding_plan_tests {
    use super::*;

    #[test]
    fn candidate_and_resolution_precedence_is_mojo_authoritative() {
        assert_eq!(
            candidate_plan(true, false, true, true, false, false).unwrap(),
            RuntimeLineageCandidatePlan {
                kind: RuntimeLineageCandidateKind::Owner,
                identity_action: RuntimeLineageIdentityAction::Set,
            }
        );
        assert_eq!(
            candidate_plan(true, false, true, true, true, false).unwrap(),
            RuntimeLineageCandidatePlan {
                kind: RuntimeLineageCandidateKind::Conflict,
                identity_action: RuntimeLineageIdentityAction::Conflict,
            }
        );
        assert_eq!(
            resolution_plan(false, 1, 0).unwrap(),
            RuntimeLineageResolutionKind::Owned
        );
        assert_eq!(
            resolution_plan(false, 0, 1).unwrap(),
            RuntimeLineageResolutionKind::Unavailable
        );
        assert_eq!(
            resolution_plan(false, 1, 1).unwrap(),
            RuntimeLineageResolutionKind::Conflict
        );
        assert_eq!(
            lookup_affinity_plan(true, false, true).unwrap(),
            RuntimeLineageLookupAffinity::Fallback
        );
        assert_eq!(
            owner_lookup_plan(RuntimeLineageOwnerKind::Owned, true, false).unwrap(),
            RuntimeLineageOwnerLookupPlan {
                owner_kind: RuntimeLineageOwnerKind::Unavailable,
                profile_action: RuntimeLineageOwnerProfileAction::ConflictSentinel,
            }
        );
    }
}

#[cfg(test)]
mod release_plan_tests {
    use super::*;

    #[test]
    fn release_plan_preserves_session_when_response_or_turn_state_is_present() {
        assert_eq!(
            release_plan(true, true, false, false, true, true, true).unwrap(),
            RuntimeLineageReleasePlan {
                response: true,
                turn_state: false,
                session: false,
                compact_session: false,
            }
        );
        assert_eq!(
            release_plan(false, false, true, true, true, true, true).unwrap(),
            RuntimeLineageReleasePlan {
                response: false,
                turn_state: true,
                session: false,
                compact_session: false,
            }
        );
    }

    #[test]
    fn release_plan_releases_session_lineage_only_without_response_or_turn_state() {
        assert_eq!(
            release_plan(false, false, false, false, true, true, true).unwrap(),
            RuntimeLineageReleasePlan {
                response: false,
                turn_state: false,
                session: true,
                compact_session: true,
            }
        );
        assert_eq!(
            release_plan(false, false, false, false, true, false, true).unwrap(),
            RuntimeLineageReleasePlan {
                response: false,
                turn_state: false,
                session: false,
                compact_session: true,
            }
        );
    }
}
