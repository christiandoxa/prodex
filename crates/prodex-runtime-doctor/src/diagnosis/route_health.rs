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

pub(super) fn runtime_doctor_route_health(
    summary: &RuntimeDoctorSummary,
) -> Vec<RuntimeDoctorRouteHealthSummary> {
    let mut routes: BTreeMap<(String, String), RuntimeDoctorRouteHealthSummary> = BTreeMap::new();

    for event in &summary.route_profile_events {
        let profile = event
            .profile
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let route = event.route.clone().unwrap_or_else(|| "unknown".to_string());
        let entry = routes
            .entry((profile.clone(), route.clone()))
            .or_insert_with(|| RuntimeDoctorRouteHealthSummary {
                profile,
                route,
                ..RuntimeDoctorRouteHealthSummary::default()
            });
        entry.event_count += 1;
        entry.latest_timestamp = event.timestamp.clone();
        entry.latest_marker = Some(event.marker.clone());
        if let Some(reason) = runtime_doctor_route_event_reason(event) {
            entry.latest_reason = Some(reason);
        }
    }

    for profile in &summary.profiles {
        for route in &profile.routes {
            let entry = routes
                .entry((profile.profile.clone(), route.route.clone()))
                .or_insert_with(|| RuntimeDoctorRouteHealthSummary {
                    profile: profile.profile.clone(),
                    route: route.route.clone(),
                    ..RuntimeDoctorRouteHealthSummary::default()
                });
            entry.health_score = Some(route.health_score);
            entry.bad_pairing_score = Some(route.bad_pairing_score);
            entry.latency_score = Some(route.performance_score);
            entry.circuit_state = Some(route.circuit_state.clone());
            entry.circuit_until = route.circuit_until;
            entry.transport_backoff_until = route.transport_backoff_until;
        }
    }

    for entry in routes.values_mut() {
        if let Some(fields) = runtime_doctor_route_marker_fields(
            summary,
            "profile_health",
            &entry.profile,
            &entry.route,
        ) {
            entry.health_score =
                runtime_doctor_parse_u32_field(fields, "score").or(entry.health_score);
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
        if let Some(fields) = runtime_doctor_route_marker_fields(
            summary,
            "profile_latency",
            &entry.profile,
            &entry.route,
        ) {
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

    let mut routes = routes.into_values().collect::<Vec<_>>();
    routes.sort_by(|left, right| {
        right
            .event_count
            .cmp(&left.event_count)
            .then_with(|| left.profile.cmp(&right.profile))
            .then_with(|| left.route.cmp(&right.route))
    });
    routes
}
