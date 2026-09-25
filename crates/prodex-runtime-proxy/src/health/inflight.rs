use std::collections::BTreeMap;

use crate::RuntimeRouteKind;

pub fn runtime_profile_inflight_sort_key(
    profile_name: &str,
    profile_inflight: &BTreeMap<String, usize>,
) -> usize {
    profile_inflight.get(profile_name).copied().unwrap_or(0)
}

pub fn runtime_profile_inflight_weight(context: &str) -> usize {
    prodex_mojo_core::runtime::profile_inflight_weight(matches!(
        context,
        "websocket_session" | "responses_http"
    ))
    .unwrap_or_else(|error| panic!("Mojo inflight weight failed: {error:?}"))
}

pub fn runtime_profile_inflight_effective_hard_limit(
    context: &str,
    configured_limit: usize,
) -> usize {
    prodex_mojo_core::runtime::profile_inflight_effective_hard_limit(
        configured_limit,
        runtime_profile_inflight_weight(context),
    )
    .unwrap_or_else(|error| panic!("Mojo inflight hard limit failed: {error:?}"))
}

pub fn runtime_profile_inflight_soft_limit(
    route_kind: RuntimeRouteKind,
    pressure_mode: bool,
    base_limit: usize,
) -> usize {
    let route_kind = match route_kind {
        RuntimeRouteKind::Responses => 0,
        RuntimeRouteKind::Compact => 1,
        RuntimeRouteKind::Websocket => 2,
        RuntimeRouteKind::Standard => 3,
    };
    prodex_mojo_core::runtime::profile_inflight_soft_limit(route_kind, pressure_mode, base_limit)
        .unwrap_or_else(|error| panic!("Mojo inflight soft limit failed: {error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflight_policy_preserves_weights_and_pressure_boundaries() {
        assert_eq!(runtime_profile_inflight_weight("responses_http"), 2);
        assert_eq!(runtime_profile_inflight_weight("websocket_session"), 2);
        assert_eq!(runtime_profile_inflight_weight("standard_http"), 1);
        assert_eq!(
            runtime_profile_inflight_effective_hard_limit("responses_http", 0),
            2
        );
        assert_eq!(
            runtime_profile_inflight_effective_hard_limit("standard_http", 0),
            1
        );
        assert_eq!(
            runtime_profile_inflight_effective_hard_limit("standard_http", usize::MAX),
            usize::MAX,
        );

        for (route, pressure_mode, base_limit, expected) in [
            (RuntimeRouteKind::Responses, false, 0, 1),
            (RuntimeRouteKind::Responses, true, 0, 1),
            (RuntimeRouteKind::Responses, true, 5, 4),
            (RuntimeRouteKind::Compact, true, 1, 1),
            (RuntimeRouteKind::Compact, true, 5, 3),
            (RuntimeRouteKind::Websocket, true, 0, 1),
            (RuntimeRouteKind::Websocket, true, 5, 4),
            (RuntimeRouteKind::Standard, true, 5, 3),
        ] {
            assert_eq!(
                runtime_profile_inflight_soft_limit(route, pressure_mode, base_limit),
                expected,
                "route={route:?} pressure={pressure_mode} base={base_limit}"
            );
        }
        assert_eq!(
            runtime_profile_inflight_soft_limit(RuntimeRouteKind::Compact, true, usize::MAX),
            usize::MAX - 2,
        );
    }
}
