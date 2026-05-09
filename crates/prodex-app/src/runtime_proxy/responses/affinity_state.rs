use super::*;

#[derive(Debug)]
pub(super) struct RuntimeResponsesAffinityState {
    bound_profile: Option<String>,
    trusted_previous_response_affinity: bool,
    turn_state_profile: Option<String>,
    route_affinity: RuntimeResponseRouteAffinity,
    previous_response_retry_candidate: Option<String>,
    previous_response_retry_index: usize,
    candidate_turn_state_retry_profile: Option<String>,
    candidate_turn_state_retry_value: Option<String>,
    saw_previous_response_not_found: bool,
}

impl RuntimeResponsesAffinityState {
    pub(super) fn new(
        bound_profile: Option<String>,
        trusted_previous_response_affinity: bool,
        turn_state_profile: Option<String>,
    ) -> Self {
        Self {
            bound_profile,
            trusted_previous_response_affinity,
            turn_state_profile,
            route_affinity: RuntimeResponseRouteAffinity::default(),
            previous_response_retry_candidate: None,
            previous_response_retry_index: 0,
            candidate_turn_state_retry_profile: None,
            candidate_turn_state_retry_value: None,
            saw_previous_response_not_found: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn refresh_route_affinity(
        &mut self,
        shared: &RuntimeRotationProxyShared,
        request_id: u64,
        reason: &str,
        previous_response_id: Option<&str>,
        request_turn_state: Option<&str>,
        request_session_id: Option<&str>,
        explicit_request_session_id: Option<&RuntimeExplicitSessionId>,
    ) -> Result<()> {
        refresh_and_log_runtime_response_route_affinity(
            shared,
            request_id,
            None,
            reason,
            previous_response_id,
            self.bound_profile.as_deref(),
            self.turn_state_profile.as_deref(),
            request_turn_state,
            request_session_id,
            explicit_request_session_id,
            None,
            &mut self.route_affinity.bound_session_profile,
            &mut self.route_affinity.compact_followup_profile,
            &mut self.route_affinity.compact_session_profile,
            &mut self.route_affinity.session_profile,
            &mut self.route_affinity.pinned_profile,
        )
    }

    pub(super) fn compact_followup_profile(&self) -> Option<(&str, &'static str)> {
        self.route_affinity
            .compact_followup_profile
            .as_ref()
            .map(|(profile_name, source)| (profile_name.as_str(), *source))
    }

    pub(super) fn compact_followup_profile_name(&self) -> Option<&str> {
        self.compact_followup_profile()
            .map(|(profile_name, _)| profile_name)
    }

    pub(super) fn compact_session_profile(&self) -> Option<&str> {
        self.route_affinity.compact_session_profile.as_deref()
    }

    pub(super) fn session_profile(&self) -> Option<&str> {
        self.route_affinity.session_profile.as_deref()
    }

    pub(super) fn pinned_profile(&self) -> Option<&str> {
        self.route_affinity.pinned_profile.as_deref()
    }

    pub(super) fn turn_state_profile(&self) -> Option<&str> {
        self.turn_state_profile.as_deref()
    }

    pub(super) fn trusted_previous_response_affinity(&self) -> bool {
        self.trusted_previous_response_affinity
    }

    pub(super) fn noncompact_session_priority_profile(&self) -> Option<&str> {
        runtime_noncompact_session_priority_profile(
            self.session_profile(),
            self.compact_session_profile(),
        )
    }

    pub(super) fn has_continuation_priority(
        &self,
        previous_response_id: Option<&str>,
        request_turn_state: Option<&str>,
    ) -> bool {
        runtime_proxy_has_continuation_priority(
            previous_response_id,
            self.pinned_profile(),
            request_turn_state,
            self.turn_state_profile(),
            self.noncompact_session_priority_profile(),
        )
    }

    pub(super) fn wait_affinity_owner(&self) -> Option<&str> {
        runtime_wait_affinity_owner(
            self.compact_followup_profile_name(),
            self.pinned_profile(),
            self.turn_state_profile(),
            self.noncompact_session_priority_profile(),
            self.trusted_previous_response_affinity,
        )
    }

    pub(super) fn allows_direct_current_profile_fallback(
        &self,
        previous_response_id: Option<&str>,
        request_turn_state: Option<&str>,
        saw_inflight_saturation: bool,
        saw_upstream_failure: bool,
    ) -> bool {
        runtime_proxy_allows_direct_current_profile_fallback(
            previous_response_id,
            self.pinned_profile(),
            request_turn_state,
            self.turn_state_profile(),
            self.noncompact_session_priority_profile(),
            saw_inflight_saturation,
            saw_upstream_failure,
        )
    }

    pub(super) fn candidate_selection<'a>(
        &'a self,
        excluded_profiles: &'a BTreeSet<String>,
        previous_response_id: Option<&'a str>,
        prompt_cache_key: Option<&'a str>,
    ) -> RuntimeResponseCandidateSelection<'a> {
        RuntimeResponseCandidateSelection {
            excluded_profiles,
            strict_affinity_profile: self.compact_followup_profile_name(),
            pinned_profile: self.pinned_profile(),
            turn_state_profile: self.turn_state_profile(),
            session_profile: self.session_profile(),
            prompt_cache_key,
            discover_previous_response_owner: previous_response_id.is_some(),
            previous_response_id,
            route_kind: RuntimeRouteKind::Responses,
        }
    }

    pub(super) fn candidate_affinity<'a>(
        &'a self,
        candidate_name: &'a str,
    ) -> RuntimeCandidateAffinity<'a> {
        RuntimeCandidateAffinity {
            route_kind: RuntimeRouteKind::Responses,
            candidate_name,
            strict_affinity_profile: self.compact_followup_profile_name(),
            pinned_profile: self.pinned_profile(),
            turn_state_profile: self.turn_state_profile(),
            session_profile: self.session_profile(),
            trusted_previous_response_affinity: self.trusted_previous_response_affinity,
        }
    }

    pub(super) fn quota_blocked_affinity_is_releasable(
        &self,
        profile_name: &str,
        request_requires_previous_response_affinity: bool,
        fresh_fallback_shape: Option<RuntimePreviousResponseFreshFallbackShape>,
    ) -> bool {
        runtime_quota_blocked_affinity_is_releasable(
            self.candidate_affinity(profile_name),
            request_requires_previous_response_affinity,
            fresh_fallback_shape,
        )
    }

    pub(super) fn turn_state_override_for<'a>(
        &'a self,
        candidate_name: &str,
        request_turn_state: Option<&'a str>,
    ) -> Option<&'a str> {
        if self.candidate_turn_state_retry_profile.as_deref() == Some(candidate_name) {
            self.candidate_turn_state_retry_value.as_deref()
        } else {
            request_turn_state
        }
    }

    pub(super) fn clear_profile_affinity(
        &mut self,
        profile_name: &str,
        reset_previous_response_retry_index: bool,
    ) {
        clear_runtime_response_profile_affinity(
            profile_name,
            &mut self.bound_profile,
            &mut self.route_affinity.session_profile,
            &mut self.candidate_turn_state_retry_profile,
            &mut self.candidate_turn_state_retry_value,
            &mut self.route_affinity.pinned_profile,
            &mut self.previous_response_retry_index,
            reset_previous_response_retry_index,
            &mut self.turn_state_profile,
            None,
        );
    }

    pub(super) fn previous_response_not_found_state<'a>(
        &'a mut self,
        excluded_profiles: &'a mut BTreeSet<String>,
        update_trusted_previous_response_affinity: bool,
    ) -> RuntimePreviousResponseNotFoundState<'a> {
        RuntimePreviousResponseNotFoundState {
            saw_previous_response_not_found: &mut self.saw_previous_response_not_found,
            previous_response_retry_candidate: &mut self.previous_response_retry_candidate,
            previous_response_retry_index: &mut self.previous_response_retry_index,
            candidate_turn_state_retry_profile: &mut self.candidate_turn_state_retry_profile,
            candidate_turn_state_retry_value: &mut self.candidate_turn_state_retry_value,
            bound_profile: &mut self.bound_profile,
            session_profile: &mut self.route_affinity.session_profile,
            pinned_profile: &mut self.route_affinity.pinned_profile,
            turn_state_profile: &mut self.turn_state_profile,
            compact_followup_profile: Some(&mut self.route_affinity.compact_followup_profile),
            excluded_profiles,
            trusted_previous_response_affinity: if update_trusted_previous_response_affinity {
                Some(&mut self.trusted_previous_response_affinity)
            } else {
                None
            },
        }
    }

    pub(super) fn remember_successful_previous_response_owner(
        &self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        previous_response_id: Option<&str>,
    ) -> Result<()> {
        if self.saw_previous_response_not_found {
            remember_runtime_successful_previous_response_owner(
                shared,
                profile_name,
                previous_response_id,
                RuntimeRouteKind::Responses,
            )?;
        }
        Ok(())
    }
}
