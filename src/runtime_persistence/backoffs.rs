use super::*;

pub(crate) fn runtime_profile_route_key_parts<'a>(
    key: &'a str,
    prefix: &str,
) -> Option<(&'a str, &'a str)> {
    prodex_runtime_store::runtime_profile_route_key_parts(key, prefix)
}

pub(crate) fn runtime_profile_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeRouteKind,
) -> String {
    prodex_runtime_store::runtime_profile_transport_backoff_key(profile_name, route_kind)
}

#[allow(dead_code)]
pub(crate) fn runtime_profile_transport_backoff_profile_name(key: &str) -> &str {
    prodex_runtime_store::runtime_profile_transport_backoff_profile_name(key)
}

pub(crate) fn runtime_profile_transport_backoff_key_valid(
    key: &str,
    valid_profiles: &BTreeSet<String>,
) -> bool {
    prodex_runtime_store::runtime_profile_transport_backoff_key_valid(key, valid_profiles)
}

pub(crate) fn runtime_profile_transport_backoff_until_from_map(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> Option<i64> {
    prodex_runtime_store::runtime_profile_transport_backoff_until_from_map(
        transport_backoff_until,
        profile_name,
        route_kind,
        now,
    )
}

#[allow(dead_code)]
pub(crate) fn runtime_profile_transport_backoff_max_until(
    transport_backoff_until: &BTreeMap<String, i64>,
    profile_name: &str,
    now: i64,
) -> Option<i64> {
    prodex_runtime_store::runtime_profile_transport_backoff_max_until(
        transport_backoff_until,
        profile_name,
        now,
    )
}
