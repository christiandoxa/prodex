use super::*;
use std::collections::BTreeMap;

fn runtime_doctor_route_event_reason(event: &RuntimeDoctorRouteProfileEvent) -> Option<String> {
    event.detail.split_whitespace().find_map(|part| {
        part.strip_prefix("reason=")
            .or_else(|| part.strip_prefix("outcome="))
            .map(ToString::to_string)
    })
}

fn runtime_doctor_route_marker_fields<'a>(
    summary: &'a RuntimeDoctorSummary,
    marker: &str,
    profile: &str,
    route: &str,
) -> Option<&'a BTreeMap<String, String>> {
    summary.marker_last_fields.get(marker).filter(|fields| {
        fields
            .get("profile")
            .is_none_or(|value| value == profile || value == "-")
            && fields
                .get("route")
                .is_none_or(|value| value == route || value == "-")
    })
}

fn runtime_doctor_parse_u32_field(fields: &BTreeMap<String, String>, key: &str) -> Option<u32> {
    fields.get(key)?.parse().ok()
}

fn runtime_doctor_parse_i64_field(fields: &BTreeMap<String, String>, key: &str) -> Option<i64> {
    fields.get(key)?.parse().ok()
}

fn runtime_doctor_latest_route_event_reason(
    summary: &RuntimeDoctorSummary,
    rows: &[(String, String, bool)],
    latest_event_index: usize,
    profile: &str,
    route: &str,
) -> Option<String> {
    for index in (0..=latest_event_index).rev() {
        let Some((event_profile, event_route, _)) = rows.get(index) else {
            continue;
        };
        if event_profile == profile
            && event_route == route
            && let Some(reason) = summary
                .route_profile_events
                .get(index)
                .and_then(runtime_doctor_route_event_reason)
        {
            return Some(reason);
        }
    }
    None
}

fn runtime_doctor_apply_route_marker_fields(
    summary: &RuntimeDoctorSummary,
    entry: &mut RuntimeDoctorRouteHealthSummary,
) {
    if let Some(fields) =
        runtime_doctor_route_marker_fields(summary, "profile_health", &entry.profile, &entry.route)
    {
        entry.health_score = runtime_doctor_parse_u32_field(fields, "score").or(entry.health_score);
        entry.health_reason = fields.get("reason").cloned();
    }
    if let Some(fields) = runtime_doctor_route_marker_fields(
        summary,
        "profile_bad_pairing",
        &entry.profile,
        &entry.route,
    ) {
        entry.bad_pairing_score =
            runtime_doctor_parse_u32_field(fields, "score").or(entry.bad_pairing_score);
        entry.bad_pairing_reason = fields.get("reason").cloned();
    }
    if let Some(fields) =
        runtime_doctor_route_marker_fields(summary, "profile_latency", &entry.profile, &entry.route)
    {
        entry.latency_score =
            runtime_doctor_parse_u32_field(fields, "score").or(entry.latency_score);
        entry.latency_reason = fields.get("reason").cloned();
    }
    if let Some(fields) = runtime_doctor_route_marker_fields(
        summary,
        "profile_circuit_open",
        &entry.profile,
        &entry.route,
    ) {
        entry.circuit_state = Some("open".to_string());
        entry.circuit_until =
            runtime_doctor_parse_i64_field(fields, "until").or(entry.circuit_until);
        entry.circuit_reason = fields.get("reason").cloned();
    }
    if let Some(fields) = runtime_doctor_route_marker_fields(
        summary,
        "profile_transport_backoff",
        &entry.profile,
        &entry.route,
    ) {
        entry.transport_backoff_until =
            runtime_doctor_parse_i64_field(fields, "until").or(entry.transport_backoff_until);
        entry.transport_backoff_context = fields
            .get("context")
            .or_else(|| fields.get("reason"))
            .cloned();
    }
}

pub(super) fn runtime_doctor_route_health(
    summary: &RuntimeDoctorSummary,
) -> Vec<RuntimeDoctorRouteHealthSummary> {
    let event_row_count = summary.route_profile_events.len();
    let mut rows = Vec::with_capacity(
        event_row_count
            + summary
                .profiles
                .iter()
                .map(|profile| profile.routes.len())
                .sum::<usize>(),
    );
    for event in &summary.route_profile_events {
        let profile = event.profile.as_deref().unwrap_or("unknown").to_string();
        let route = event.route.clone().unwrap_or_else(|| "unknown".to_string());
        rows.push((profile, route, true));
    }

    let mut profile_rows = Vec::new();
    for profile in &summary.profiles {
        for route in &profile.routes {
            rows.push((profile.profile.clone(), route.route.clone(), false));
            profile_rows.push(RuntimeDoctorRouteHealthSummary {
                profile: profile.profile.clone(),
                route: route.route.clone(),
                health_score: Some(route.health_score),
                bad_pairing_score: Some(route.bad_pairing_score),
                latency_score: Some(route.performance_score),
                circuit_state: Some(route.circuit_state.clone()),
                circuit_until: route.circuit_until,
                transport_backoff_until: route.transport_backoff_until,
                ..RuntimeDoctorRouteHealthSummary::default()
            });
        }
    }

    let groups = prodex_mojo_core::rich::runtime_doctor_route_health_groups(
        rows.iter()
            .map(|(profile, route, is_event)| (profile.as_str(), route.as_str(), *is_event)),
    )
    .expect("Mojo runtime-doctor route-health reducer returned invalid output");
    let mut routes = Vec::with_capacity(groups.len());
    for group in groups {
        let (profile, route, _) = rows
            .get(group.representative_index)
            .expect("Mojo runtime-doctor route-health representative is in range");
        let mut entry = RuntimeDoctorRouteHealthSummary {
            profile: profile.clone(),
            route: route.clone(),
            event_count: group.event_count,
            ..RuntimeDoctorRouteHealthSummary::default()
        };
        if let Some(event_index) = group.latest_event_index {
            let event = summary
                .route_profile_events
                .get(event_index)
                .expect("Mojo runtime-doctor latest event is in range");
            entry.latest_timestamp = event.timestamp.clone();
            entry.latest_marker = Some(event.marker.clone());
            entry.latest_reason = runtime_doctor_latest_route_event_reason(
                summary,
                &rows,
                event_index,
                profile,
                route,
            );
        }
        if let Some(profile_row_index) = group.profile_row_index {
            let profile_row = profile_row_index
                .checked_sub(event_row_count)
                .and_then(|index| profile_rows.get(index))
                .expect("Mojo runtime-doctor profile row is in range");
            entry.health_score = profile_row.health_score;
            entry.bad_pairing_score = profile_row.bad_pairing_score;
            entry.latency_score = profile_row.latency_score;
            entry.circuit_state = profile_row.circuit_state.clone();
            entry.circuit_until = profile_row.circuit_until;
            entry.transport_backoff_until = profile_row.transport_backoff_until;
        }
        runtime_doctor_apply_route_marker_fields(summary, &mut entry);
        routes.push(entry);
    }
    routes
}
