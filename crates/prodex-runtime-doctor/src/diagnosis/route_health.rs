use super::*;
use std::collections::BTreeMap;

fn runtime_doctor_route_marker_fields<'a>(
    summary: &'a RuntimeDoctorSummary,
    marker: &str,
    profile: &str,
    route: &str,
) -> Option<&'a BTreeMap<String, String>> {
    summary.marker_last_fields.get(marker).filter(|fields| {
        prodex_mojo_core::rich::runtime_doctor_route_marker_matches(
            fields.get("profile").map(String::as_str),
            fields.get("route").map(String::as_str),
            profile,
            route,
        )
        .expect("Mojo route-health marker scope returned invalid output")
    })
}

fn runtime_doctor_parse_u32_field(fields: &BTreeMap<String, String>, key: &str) -> Option<u32> {
    prodex_mojo_core::rich::runtime_doctor_route_marker_number(
        fields.get(key).map(String::as_str),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_ROUTE_MARKER_U32,
    )
    .expect("Mojo route-health u32 marker returned invalid output")
    .map(|score| u32::try_from(score).expect("validated Mojo score fits u32"))
}

fn runtime_doctor_parse_i64_field(fields: &BTreeMap<String, String>, key: &str) -> Option<i64> {
    prodex_mojo_core::rich::runtime_doctor_route_marker_number(
        fields.get(key).map(String::as_str),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_ROUTE_MARKER_I64,
    )
    .expect("Mojo route-health i64 marker returned invalid output")
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
    let reason_events = summary
        .route_profile_events
        .iter()
        .map(
            |event| prodex_mojo_core::rich::RuntimeDoctorRouteReasonEvent {
                profile: event.profile.as_deref().unwrap_or("unknown"),
                route: event.route.as_deref().unwrap_or("unknown"),
                detail: &event.detail,
            },
        )
        .collect::<Vec<_>>();
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
            entry.latest_reason = prodex_mojo_core::rich::runtime_doctor_route_latest_reason(
                &reason_events,
                event_index,
                profile,
                route,
            )
            .expect("Mojo runtime-doctor latest route reason returned invalid output")
            .map(str::to_string);
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

#[cfg(test)]
mod mojo_marker_field_tests {
    use super::*;

    #[test]
    fn route_marker_fields_honor_wildcards_and_checked_integer_boundaries() {
        let mut summary = RuntimeDoctorSummary::default();
        summary.marker_last_fields.insert(
            "profile_health".into(),
            BTreeMap::from([
                ("profile".into(), "-".into()),
                ("route".into(), "-".into()),
                ("score".into(), "+4294967295".into()),
                ("reason".into(), "📈".into()),
            ]),
        );
        summary.marker_last_fields.insert(
            "profile_circuit_open".into(),
            BTreeMap::from([
                ("profile".into(), "東京".into()),
                ("route".into(), "responses".into()),
                ("until".into(), i64::MIN.to_string()),
            ]),
        );
        summary.marker_last_fields.insert(
            "profile_transport_backoff".into(),
            BTreeMap::from([
                ("profile".into(), "beta".into()),
                ("until".into(), "9223372036854775808".into()),
            ]),
        );
        let mut entry = RuntimeDoctorRouteHealthSummary {
            profile: "東京".into(),
            route: "responses".into(),
            health_score: Some(7),
            transport_backoff_until: Some(15),
            ..RuntimeDoctorRouteHealthSummary::default()
        };
        runtime_doctor_apply_route_marker_fields(&summary, &mut entry);
        assert_eq!(entry.health_score, Some(u32::MAX));
        assert_eq!(entry.health_reason.as_deref(), Some("📈"));
        assert_eq!(entry.circuit_state.as_deref(), Some("open"));
        assert_eq!(entry.circuit_until, Some(i64::MIN));
        assert_eq!(entry.transport_backoff_until, Some(15));

        let mut unmatched = RuntimeDoctorRouteHealthSummary {
            profile: "alpha".into(),
            route: "websocket".into(),
            health_score: Some(2),
            ..RuntimeDoctorRouteHealthSummary::default()
        };
        runtime_doctor_apply_route_marker_fields(&summary, &mut unmatched);
        assert_eq!(unmatched.health_score, Some(u32::MAX));
        assert_eq!(unmatched.circuit_state, None);
    }
}
