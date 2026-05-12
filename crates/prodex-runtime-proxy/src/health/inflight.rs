use std::collections::BTreeMap;

use crate::RuntimeRouteKind;

pub fn runtime_profile_inflight_sort_key(
    profile_name: &str,
    profile_inflight: &BTreeMap<String, usize>,
) -> usize {
    profile_inflight.get(profile_name).copied().unwrap_or(0)
}

pub fn runtime_profile_inflight_weight(context: &str) -> usize {
    match context {
        "websocket_session" | "responses_http" => 2,
        _ => 1,
    }
}

pub fn runtime_profile_inflight_soft_limit(
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
