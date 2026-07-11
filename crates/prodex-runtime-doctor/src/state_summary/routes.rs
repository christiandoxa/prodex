use std::collections::BTreeMap;

use super::{
    RuntimeDoctorBackoffMaps, RuntimeDoctorHealthScore, RuntimeDoctorRouteKind,
    RuntimeDoctorStateSummaryConfig,
};

pub fn runtime_doctor_route_kind_label(route_kind: RuntimeDoctorRouteKind) -> &'static str {
    match route_kind {
        RuntimeDoctorRouteKind::Responses => "responses",
        RuntimeDoctorRouteKind::Websocket => "websocket",
        RuntimeDoctorRouteKind::Compact => "compact",
        RuntimeDoctorRouteKind::Standard => "standard",
    }
}

fn runtime_doctor_route_key_parts<'a>(key: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (route, profile_name) = rest.split_once(':')?;
    Some((route, profile_name))
}

pub(super) fn runtime_doctor_route_health_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_health__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_doctor_route_bad_pairing_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_bad_pairing__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_doctor_route_performance_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_performance__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_doctor_route_circuit_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_circuit__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

pub(super) fn runtime_doctor_transport_backoff_key(
    profile_name: &str,
    route_kind: RuntimeDoctorRouteKind,
) -> String {
    format!(
        "__route_transport_backoff__:{}:{profile_name}",
        runtime_doctor_route_kind_label(route_kind)
    )
}

fn runtime_doctor_transport_backoff_key_parts(key: &str) -> Option<(&str, &str)> {
    runtime_doctor_route_key_parts(key, "__route_transport_backoff__:")
}

pub(super) fn runtime_doctor_transport_backoff_profile_name(key: &str) -> &str {
    runtime_doctor_transport_backoff_key_parts(key)
        .map(|(_, profile_name)| profile_name)
        .unwrap_or(key)
}

fn runtime_doctor_effective_score(
    entry: &RuntimeDoctorHealthScore,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    let decay = now
        .saturating_sub(entry.updated_at)
        .saturating_div(decay_seconds.max(1))
        .clamp(0, i64::from(u32::MAX)) as u32;
    entry.score.saturating_sub(decay)
}

pub(super) fn runtime_doctor_effective_health_score(
    entry: &RuntimeDoctorHealthScore,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> u32 {
    runtime_doctor_effective_score(entry, now, config.health_decay_seconds)
}

pub(super) fn runtime_doctor_effective_health_score_from_map(
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    key: &str,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> u32 {
    scores
        .get(key)
        .map(|entry| runtime_doctor_effective_health_score(entry, now, config))
        .unwrap_or(0)
}

pub(super) fn runtime_doctor_effective_score_from_map(
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    key: &str,
    now: i64,
    decay_seconds: i64,
) -> u32 {
    scores
        .get(key)
        .map(|entry| runtime_doctor_effective_score(entry, now, decay_seconds))
        .unwrap_or(0)
}

fn runtime_doctor_push_route_circuits(
    routes: &mut Vec<String>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    now: i64,
) {
    for (key, until) in backoffs.route_circuit_open_until {
        if let Some((route, profile_name)) =
            runtime_doctor_route_key_parts(key, "__route_circuit__:")
        {
            let state = if *until > now { "open" } else { "half-open" };
            routes.push(format!(
                "{profile_name}/{route} circuit={state} until={until}"
            ));
        }
    }
}

fn runtime_doctor_push_transport_backoffs(
    routes: &mut Vec<String>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
) {
    for (profile_name, until) in backoffs.transport_backoff_until {
        if let Some((route, profile_name)) =
            runtime_doctor_transport_backoff_key_parts(profile_name)
        {
            routes.push(format!(
                "{profile_name}/{route} transport_backoff until={until}"
            ));
        } else {
            routes.push(format!(
                "{profile_name}/transport transport_backoff until={until}"
            ));
        }
    }
}

fn runtime_doctor_push_retry_backoffs(
    routes: &mut Vec<String>,
    backoffs: RuntimeDoctorBackoffMaps<'_>,
) {
    for (profile_name, until) in backoffs.retry_backoff_until {
        routes.push(format!("{profile_name}/retry retry_backoff until={until}"));
    }
}

fn runtime_doctor_push_score_degradations(
    routes: &mut Vec<String>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) {
    for (key, health) in scores {
        if let Some((route, profile_name)) =
            runtime_doctor_route_key_parts(key, "__route_bad_pairing__:")
        {
            let score =
                runtime_doctor_effective_score(health, now, config.bad_pairing_decay_seconds);
            if score > 0 {
                routes.push(format!("{profile_name}/{route} bad_pairing={score}"));
            }
            continue;
        }
        if let Some((route, profile_name)) =
            runtime_doctor_route_key_parts(key, "__route_health__:")
        {
            let score = runtime_doctor_effective_health_score(health, now, config);
            if score > 0 {
                routes.push(format!("{profile_name}/{route} health={score}"));
            }
        }
    }
}

pub fn runtime_doctor_degraded_routes(
    backoffs: RuntimeDoctorBackoffMaps<'_>,
    scores: &BTreeMap<String, RuntimeDoctorHealthScore>,
    now: i64,
    config: RuntimeDoctorStateSummaryConfig,
) -> Vec<String> {
    let mut routes = Vec::new();
    runtime_doctor_push_route_circuits(&mut routes, backoffs, now);
    runtime_doctor_push_transport_backoffs(&mut routes, backoffs);
    runtime_doctor_push_retry_backoffs(&mut routes, backoffs);
    runtime_doctor_push_score_degradations(&mut routes, scores, now, config);
    routes.sort();
    routes.dedup();
    routes.truncate(8);
    routes
}
