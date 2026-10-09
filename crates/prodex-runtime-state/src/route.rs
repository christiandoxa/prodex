use std::collections::BTreeSet;

use crate::RuntimeRouteKind;

pub fn runtime_route_kind_label(route_kind: RuntimeRouteKind) -> &'static str {
    prodex_mojo_core::runtime_state::route_kind_label(route_kind as u8)
        .expect("Mojo runtime route label policy returned invalid output")
}

pub fn runtime_route_kind_from_label(label: &str) -> Option<RuntimeRouteKind> {
    let kind = prodex_mojo_core::runtime_state::route_kind_from_label(label)
        .expect("Mojo runtime route label parser returned invalid output");
    kind.map(runtime_route_kind_from_tag)
}

pub fn runtime_route_coupled_kinds(route_kind: RuntimeRouteKind) -> &'static [RuntimeRouteKind] {
    const RESPONSES: &[RuntimeRouteKind] = &[RuntimeRouteKind::Websocket];
    const COMPACT: &[RuntimeRouteKind] = &[RuntimeRouteKind::Standard];
    const WEBSOCKET: &[RuntimeRouteKind] = &[RuntimeRouteKind::Responses];
    const STANDARD: &[RuntimeRouteKind] = &[RuntimeRouteKind::Compact];

    match prodex_mojo_core::runtime_state::route_coupled_kind(route_kind as u8)
        .expect("Mojo runtime route coupling policy returned invalid output")
    {
        0 => WEBSOCKET,
        1 => STANDARD,
        2 => RESPONSES,
        3 => COMPACT,
        _ => unreachable!("validated Mojo runtime route kind"),
    }
}

fn runtime_route_kind_from_tag(tag: u8) -> RuntimeRouteKind {
    match tag {
        0 => RuntimeRouteKind::Responses,
        1 => RuntimeRouteKind::Compact,
        2 => RuntimeRouteKind::Websocket,
        3 => RuntimeRouteKind::Standard,
        _ => unreachable!("validated Mojo runtime route kind"),
    }
}

fn runtime_profile_route_key(
    key_kind: u8,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    prodex_mojo_core::runtime_state::route_key(key_kind, route_kind as u8, profile_name)
        .expect("Mojo runtime route key policy returned invalid output")
}

pub fn runtime_profile_route_health_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    runtime_profile_route_key(
        prodex_mojo_core::runtime_state::ROUTE_KEY_HEALTH,
        profile_name,
        route_kind,
    )
}

pub fn runtime_profile_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    runtime_profile_route_key(
        prodex_mojo_core::runtime_state::ROUTE_KEY_BAD_PAIRING,
        profile_name,
        route_kind,
    )
}

pub fn runtime_profile_route_success_streak_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    runtime_profile_route_key(
        prodex_mojo_core::runtime_state::ROUTE_KEY_SUCCESS_STREAK,
        profile_name,
        route_kind,
    )
}

pub fn runtime_profile_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    runtime_profile_route_key(
        prodex_mojo_core::runtime_state::ROUTE_KEY_PERFORMANCE,
        profile_name,
        route_kind,
    )
}

pub fn runtime_profile_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    runtime_profile_route_key(
        prodex_mojo_core::runtime_state::ROUTE_KEY_CIRCUIT,
        profile_name,
        route_kind,
    )
}

pub fn runtime_profile_route_circuit_profile_name(key: &str) -> &str {
    let (start, end) = prodex_mojo_core::runtime_state::route_profile_suffix_span(key)
        .expect("Mojo runtime route suffix policy returned invalid output");
    key.get(start..end)
        .expect("Mojo runtime route suffix returned invalid UTF-8 span")
}

pub fn runtime_profile_route_circuit_health_key(key: &str) -> String {
    prodex_mojo_core::runtime_state::route_circuit_health_key(key)
        .expect("Mojo runtime route circuit-health policy returned invalid output")
}

pub fn runtime_profile_route_circuit_reopen_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    runtime_profile_route_key(
        prodex_mojo_core::runtime_state::ROUTE_KEY_CIRCUIT_REOPEN,
        profile_name,
        route_kind,
    )
}

pub fn runtime_profile_route_key_parts<'a>(
    key: &'a str,
    prefix: &str,
) -> Option<(&'a str, &'a str)> {
    let (route_start, route_end, profile_start, profile_end) =
        prodex_mojo_core::runtime_state::route_key_parts(key, prefix)
            .expect("Mojo runtime route parser returned invalid output")?;
    Some((
        key.get(route_start..route_end)?,
        key.get(profile_start..profile_end)?,
    ))
}

pub fn runtime_profile_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    runtime_profile_route_key(
        prodex_mojo_core::runtime_state::ROUTE_KEY_TRANSPORT_BACKOFF,
        profile_name,
        route_kind,
    )
}

pub fn runtime_profile_transport_backoff_key_parts(key: &str) -> Option<(&str, &str)> {
    runtime_profile_route_key_parts(key, "__route_transport_backoff__:")
}

pub fn runtime_profile_transport_backoff_profile_name(key: &str) -> &str {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(_, profile_name)| profile_name)
        .unwrap_or(key)
}

pub fn runtime_profile_transport_backoff_key_valid(
    key: &str,
    valid_profiles: &BTreeSet<String>,
) -> bool {
    runtime_profile_transport_backoff_key_parts(key)
        .map(|(route, profile_name)| {
            runtime_route_kind_from_label(route).is_some() && valid_profiles.contains(profile_name)
        })
        .unwrap_or_else(|| valid_profiles.contains(key))
}
