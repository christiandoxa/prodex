//! Compact-route affinity predicates.

use super::super::super::{RuntimeCandidateAffinity, runtime_candidate_has_hard_affinity};
use crate::runtime_state_shared::RuntimeRouteKind;

pub(super) fn runtime_compact_candidate_has_hard_affinity(
    candidate_name: &str,
    strict_affinity_profile: Option<&str>,
    session_profile: Option<&str>,
) -> bool {
    runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
        route_kind: RuntimeRouteKind::Compact,
        candidate_name,
        strict_affinity_profile,
        pinned_profile: None,
        turn_state_profile: None,
        session_profile,
        trusted_previous_response_affinity: false,
    })
}

pub(super) fn runtime_compact_route_candidate_has_hard_affinity(
    candidate_name: &str,
    compact_followup_profile: &Option<(String, &'static str)>,
    session_profile: Option<&str>,
) -> bool {
    runtime_compact_candidate_has_hard_affinity(
        candidate_name,
        compact_followup_profile
            .as_ref()
            .map(|(profile_name, _)| profile_name.as_str()),
        session_profile,
    )
}
