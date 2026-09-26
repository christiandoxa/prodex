use super::*;

#[test]
fn candidate_plan_handles_pools_larger_than_mojo_batch_limit() {
    for count in [257, 513] {
        let names = (0..count)
            .map(|index| format!("profile-{index:04}"))
            .collect::<Vec<_>>();
        let owner = names.last().unwrap();
        let affinity = runtime_prompt_cache_affinity_batch(
            Some("workspace-cache"),
            Some(owner),
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .unwrap();
        let inflight_counts = (0..count)
            .map(|index| if [0, 1, 2, 4].contains(&index) { 3 } else { 0 })
            .collect::<Vec<_>>();
        let mut exhausted = healthy_quota_summary();
        exhausted.five_hour.status = RuntimeSelectionQuotaWindowStatus::Exhausted;
        exhausted.route_band = RuntimeSelectionQuotaPressureBand::Exhausted;
        let mut unknown = healthy_quota_summary();
        unknown.five_hour.status = RuntimeSelectionQuotaWindowStatus::Unknown;
        unknown.route_band = RuntimeSelectionQuotaPressureBand::Unknown;
        let candidates = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                candidate(
                    name,
                    CandidateFixture {
                        order_index: index,
                        inflight_count: inflight_counts[index],
                        auth_failure_active: index == 0,
                        quota_summary: if index == 1 {
                            exhausted
                        } else if index == 3 {
                            unknown
                        } else {
                            healthy_quota_summary()
                        },
                        in_selection_backoff: index == 2,
                        ..CandidateFixture::default()
                    },
                )
            })
            .collect();
        let plan = build_runtime_response_candidate_execution_plan(
            candidates,
            &BTreeSet::new(),
            runtime_response_candidate_plan_options(
                RuntimeRouteKind::Responses,
                3,
                Some("workspace-cache"),
                Some(owner),
                2,
            ),
        );

        let mut expected_fallback = (0..count).collect::<Vec<_>>();
        expected_fallback.sort_by_key(|index| (inflight_counts[*index], affinity[*index], *index));
        let expected_ready = expected_fallback
            .iter()
            .copied()
            .filter(|index| *index != 2)
            .collect::<Vec<_>>();
        assert_eq!(
            plan.ready_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            expected_ready
                .iter()
                .map(|index| names[*index].as_str())
                .collect::<Vec<_>>(),
            "ready order for {count} candidates"
        );
        assert_eq!(
            plan.fallback_candidates
                .iter()
                .map(|candidate| candidate.name.as_str())
                .collect::<Vec<_>>(),
            expected_fallback
                .iter()
                .map(|index| names[*index].as_str())
                .collect::<Vec<_>>(),
            "fallback order for {count} candidates"
        );
        assert_eq!(plan.ready_candidates.len(), count - 1);
        assert_eq!(plan.fallback_candidates.len(), count);

        let find = |candidates: &[RuntimeResponsePlannedCandidate], index: usize| {
            candidates
                .iter()
                .position(|candidate| candidate.name == names[index])
                .unwrap()
        };
        let auth = &plan.ready_candidates[find(&plan.ready_candidates, 0)];
        assert_eq!(auth.ready_skip_reason(), Some("auth_failure_backoff"));
        assert!(auth.inflight_soft_limited);
        assert_eq!(
            plan.ready_candidates[find(&plan.ready_candidates, 1)].quota_guard_reason,
            Some("quota_exhausted_before_send")
        );
        assert!(plan.ready_candidates[find(&plan.ready_candidates, 1)].inflight_soft_limited);
        assert_eq!(
            plan.fallback_candidates[find(&plan.fallback_candidates, 2)].ready_skip_reason(),
            Some("selection_backoff")
        );
        assert!(plan.fallback_candidates[find(&plan.fallback_candidates, 2)].inflight_soft_limited);
        assert_eq!(
            plan.ready_candidates[find(&plan.ready_candidates, 3)].availability,
            RuntimeProfileAvailabilityState::Unknown
        );
        assert_eq!(
            plan.ready_candidates[find(&plan.ready_candidates, 4)].ready_skip_reason(),
            Some("profile_inflight_soft_limit")
        );
        assert!(plan.ready_candidates[find(&plan.ready_candidates, 4)].inflight_soft_limited);
        assert_eq!(
            plan.ready_candidates[find(&plan.ready_candidates, count - 1)]
                .name
                .as_str(),
            owner.as_str()
        );
    }
}
