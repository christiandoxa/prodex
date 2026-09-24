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

// Test-only pre-migration Rust oracle retained for direct Mojo parity checks.

#[cfg(test)]
fn runtime_profile_inflight_weight_rust(context: &str) -> usize {
    match context {
        "websocket_session" | "responses_http" => 2,
        _ => 1,
    }
}

#[cfg(test)]
fn runtime_profile_inflight_soft_limit_rust(
    route_kind: RuntimeRouteKind,
    pressure_mode: bool,
    base_limit: usize,
) -> usize {
    let base = base_limit.max(1);
    if !pressure_mode {
        return base;
    }
    match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => base.saturating_sub(1).max(1),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => base.saturating_sub(2).max(1),
    }
}

#[cfg(test)]
mod mojo_parity_tests {
    use super::*;

    #[test]
    fn inflight_policy_matches_rust_oracle() {
        for context in [
            "websocket_session",
            "responses_http",
            "standard_http",
            "",
            "other",
        ] {
            assert_eq!(
                runtime_profile_inflight_weight(context),
                runtime_profile_inflight_weight_rust(context),
            );
            for limit in [0_usize, 1, 2, 8, usize::MAX / 2] {
                assert_eq!(
                    runtime_profile_inflight_effective_hard_limit(context, limit),
                    limit.max(runtime_profile_inflight_weight_rust(context)),
                );
            }
        }
        for route in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Standard,
        ] {
            for pressure in [false, true] {
                for base in [0_usize, 1, 2, 5, 64] {
                    assert_eq!(
                        runtime_profile_inflight_soft_limit(route, pressure, base),
                        runtime_profile_inflight_soft_limit_rust(route, pressure, base),
                    );
                }
            }
        }
    }

    #[test]
    fn inflight_limits_match_rust_oracles_at_usize_boundaries() {
        let mut limits = vec![0, 1, 2, 8, usize::MAX / 2, usize::MAX];
        if let Ok(signed_max) = usize::try_from(i64::MAX) {
            limits.push(signed_max);
            if signed_max < usize::MAX {
                limits.push(signed_max + 1);
            }
        }
        for context in [
            "websocket_session",
            "responses_http",
            "other",
            "RESPONSES_HTTP",
        ] {
            let weight = runtime_profile_inflight_weight_rust(context);
            for limit in &limits {
                assert_eq!(
                    runtime_profile_inflight_effective_hard_limit(context, *limit),
                    (*limit).max(weight),
                    "context={context:?} limit={limit}"
                );
            }
        }

        for route in [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Standard,
        ] {
            for pressure in [false, true] {
                for base in &limits {
                    assert_eq!(
                        runtime_profile_inflight_soft_limit(route, pressure, *base),
                        runtime_profile_inflight_soft_limit_rust(route, pressure, *base),
                        "route={route:?} pressure={pressure} base={base}"
                    );
                }
            }
        }
    }
}
