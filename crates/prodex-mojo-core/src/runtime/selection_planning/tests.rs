#[cfg(test)]
mod waitable_candidate_tests {
    use super::super::*;

    #[test]
    fn waitable_candidate_modes_preserve_expected_gate_semantics() {
        let base = WaitableCandidateInput {
            context_allowed: true,
            auth_compatible: true,
            supports_runtime: true,
            cached_probe_present: false,
            soft_limited: false,
            in_selection_backoff: false,
            auth_failure_active: false,
            health_penalized: false,
            hard_limited: false,
            snapshot_blocks: false,
            quota_blocked: false,
        };
        assert!(waitable_candidate_eligible(WaitableCandidateMode::ColdStart, base).unwrap());
        assert!(
            !waitable_candidate_eligible(
                WaitableCandidateMode::ColdStart,
                WaitableCandidateInput {
                    hard_limited: true,
                    ..base
                },
            )
            .unwrap()
        );
        assert!(
            waitable_candidate_eligible(
                WaitableCandidateMode::Waitable,
                WaitableCandidateInput {
                    hard_limited: true,
                    ..base
                },
            )
            .unwrap()
        );
        assert!(waitable_candidate_eligible(WaitableCandidateMode::Relieved, base).unwrap());
        assert!(
            !waitable_candidate_eligible(
                WaitableCandidateMode::Relieved,
                WaitableCandidateInput {
                    quota_blocked: true,
                    ..base
                },
            )
            .unwrap()
        );
    }
}

#[cfg(test)]
mod quota_selection_tests {
    use super::super::*;

    fn input(
        route_kind: i64,
        five_hour_status: i64,
        weekly_status: i64,
        quota_band: i64,
        quota_source_present: bool,
        responses_critical_floor_percent: i64,
    ) -> QuotaSelectionPolicyInput {
        QuotaSelectionPolicyInput {
            route_kind,
            five_hour_status,
            weekly_status,
            quota_band,
            quota_source_present,
            responses_critical_floor_percent,
        }
    }

    #[test]
    fn quota_selection_policy_keeps_authoritative_window_contract() {
        let healthy = input(0, 0, 0, 0, true, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_PRECOMMIT_FLOOR, healthy).unwrap(),
            10
        );
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_SUMMARY_ALLOWS, healthy).unwrap(),
            1
        );

        for (status, usable) in [(0, 1), (1, 1), (2, 1), (3, 0), (4, 0)] {
            assert_eq!(
                quota_selection_policy(
                    QUOTA_SELECTION_MODE_WINDOW_USABLE,
                    input(0, status, 0, 0, false, 10),
                )
                .unwrap(),
                usable
            );
        }

        let critical = input(0, 2, 0, 2, true, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_PRECOMMIT_REASON, critical).unwrap(),
            SOFT_AFFINITY_POLICY_ALLOWED
        );

        let weekly_exhausted = input(0, 0, 3, 3, true, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_REJECTION_REASON, weekly_exhausted)
                .unwrap(),
            SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED
        );

        let five_hour_exhausted = input(0, 3, 0, 3, true, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_PRECOMMIT_REASON, five_hour_exhausted)
                .unwrap(),
            SOFT_AFFINITY_POLICY_QUOTA_EXHAUSTED_BEFORE_SEND
        );

        let unknown = input(2, 4, 0, 4, false, 10);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_REJECTION_REASON, unknown).unwrap(),
            SOFT_AFFINITY_POLICY_QUOTA_WINDOWS_UNAVAILABLE
        );

        let compact = input(1, 0, 0, 0, true, 99);
        assert_eq!(
            quota_selection_policy(QUOTA_SELECTION_MODE_PRECOMMIT_FLOOR, compact).unwrap(),
            1
        );
    }
}

#[cfg(test)]
mod websocket_invalid_previous_response_tests {
    use super::super::*;

    #[test]
    fn websocket_invalid_previous_response_plan_preserves_recovery_precedence() {
        let reconnect =
            websocket_invalid_previous_response_plan(true, true, true, true, false).unwrap();
        assert!(reconnect.recovery_signal);
        assert!(reconnect.crossed_transport_generation);
        assert_eq!(
            reconnect.chain_reuse_reason,
            WebsocketChainReuseReason::UpstreamReconnect
        );
        assert_eq!(
            reconnect.action,
            WebsocketInvalidPreviousResponseAction::FullContextRetry
        );

        let bound = websocket_invalid_previous_response_plan(true, true, true, true, true).unwrap();
        assert_eq!(
            bound.chain_reuse_reason,
            WebsocketChainReuseReason::BoundProfileAffinity
        );

        let unbound =
            websocket_invalid_previous_response_plan(true, true, false, false, false).unwrap();
        assert!(!unbound.recovery_signal);
        assert_eq!(
            unbound.chain_reuse_reason,
            WebsocketChainReuseReason::UnboundPreviousResponse
        );
        assert_eq!(
            unbound.action,
            WebsocketInvalidPreviousResponseAction::PassThrough
        );
    }
}

#[cfg(test)]
mod noncompact_failure_plan_tests {
    use super::super::*;

    #[test]
    fn noncompact_failure_plan_preserves_terminal_rotation_policy() {
        let rate_owned =
            noncompact_failure_plan(NoncompactFailureKind::RateLimited, true, false, false)
                .unwrap();
        assert!(rate_owned.terminal);
        assert!(rate_owned.mark_backoff);
        assert!(!rate_owned.exclude_profile);

        let retry_no_fallback =
            noncompact_failure_plan(NoncompactFailureKind::Retryable, false, false, false).unwrap();
        assert!(retry_no_fallback.terminal);
        assert!(retry_no_fallback.mark_backoff);
        assert!(retry_no_fallback.clear_session);

        let overload =
            noncompact_failure_plan(NoncompactFailureKind::Retryable, false, true, false).unwrap();
        assert!(!overload.terminal);
        assert!(overload.exclude_profile);
        assert!(!overload.last_failure_retryable);

        let transport_owned =
            noncompact_failure_plan(NoncompactFailureKind::Transport, true, false, false).unwrap();
        assert!(transport_owned.terminal);
        assert!(!transport_owned.record_transport_failure);
    }
}
