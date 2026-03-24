mod prodex_impl {
    #![allow(dead_code, unused_imports)]

    include!("../src/main.rs");

    mod tests {
        #![allow(dead_code, unused_imports)]

        use tungstenite::connect as ws_connect;

        fn runtime_proxy_lane_admission_for_global_limit(
            global_limit: usize,
        ) -> RuntimeProxyLaneAdmission {
            RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
                responses: global_limit.max(1),
                compact: global_limit.max(1),
                websocket: global_limit.max(1),
                standard: global_limit.max(1),
            })
        }

        fn runtime_proxy_in_local_overload_backoff(shared: &RuntimeRotationProxyShared) -> bool {
            let now = Local::now().timestamp().max(0) as u64;
            shared.local_overload_backoff_until.load(Ordering::SeqCst) > now
        }

        fn select_runtime_response_candidate(
            shared: &RuntimeRotationProxyShared,
            excluded_profiles: &BTreeSet<String>,
            pinned_profile: Option<&str>,
            turn_state_profile: Option<&str>,
            session_profile: Option<&str>,
            discover_previous_response_owner: bool,
        ) -> Result<Option<String>> {
            select_runtime_response_candidate_for_route(
                shared,
                excluded_profiles,
                pinned_profile,
                turn_state_profile,
                session_profile,
                discover_previous_response_owner,
                RuntimeRouteKind::Responses,
            )
        }

        fn runtime_proxy_optimistic_current_candidate(
            shared: &RuntimeRotationProxyShared,
            excluded_profiles: &BTreeSet<String>,
        ) -> Result<Option<String>> {
            runtime_proxy_optimistic_current_candidate_for_route(
                shared,
                excluded_profiles,
                RuntimeRouteKind::Responses,
            )
        }

        fn next_runtime_response_candidate(
            shared: &RuntimeRotationProxyShared,
            excluded_profiles: &BTreeSet<String>,
        ) -> Result<Option<String>> {
            next_runtime_response_candidate_for_route(
                shared,
                excluded_profiles,
                RuntimeRouteKind::Responses,
            )
        }

        include!("support/main_internal_body.rs");
    }
}
