use super::*;
use std::collections::BTreeMap;

pub fn runtime_doctor_marker_count(summary: &RuntimeDoctorSummary, marker: &'static str) -> usize {
    summary.marker_counts.get(marker).copied().unwrap_or(0)
}

pub(super) fn runtime_doctor_has_any_markers(
    summary: &RuntimeDoctorSummary,
    markers: &[&'static str],
) -> bool {
    markers
        .iter()
        .any(|marker| runtime_doctor_marker_count(summary, marker) > 0)
}

pub(super) fn runtime_doctor_facet_count(
    summary: &RuntimeDoctorSummary,
    facet: &str,
    value: &str,
) -> usize {
    summary
        .facet_counts
        .get(facet)
        .and_then(|counts| counts.get(value))
        .copied()
        .unwrap_or(0)
}

pub(super) fn runtime_doctor_marker_last_field<'a>(
    summary: &'a RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<&'a str> {
    summary
        .marker_last_fields
        .get(marker)
        .and_then(|fields| fields.get(field))
        .map(String::as_str)
}

pub(super) fn runtime_doctor_marker_last_usize_field(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<usize> {
    runtime_doctor_marker_last_field(summary, marker, field)?
        .parse()
        .ok()
}

pub(super) fn runtime_doctor_marker_last_u64_field(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    field: &str,
) -> Option<u64> {
    runtime_doctor_marker_last_field(summary, marker, field)?
        .parse()
        .ok()
}

pub(super) fn runtime_doctor_marker_scope(
    summary: &RuntimeDoctorSummary,
    marker: &str,
    profile_field: &str,
    route_field: &str,
) -> Option<String> {
    let profile = runtime_doctor_marker_last_field(summary, marker, profile_field);
    let route = runtime_doctor_marker_last_field(summary, marker, route_field);
    match (profile, route) {
        (Some(profile), Some(route)) => Some(format!("{profile}/{route}")),
        (Some(profile), None) => Some(profile.to_string()),
        (None, Some(route)) => Some(route.to_string()),
        (None, None) => None,
    }
}

pub(super) fn runtime_doctor_admission_pressure_load(
    summary: &RuntimeDoctorSummary,
    marker: &str,
) -> String {
    match (
        runtime_doctor_marker_last_field(summary, marker, "active"),
        runtime_doctor_marker_last_field(summary, marker, "limit"),
    ) {
        (Some(active), Some(limit)) => format!(" Latest load: {active}/{limit}."),
        _ => String::new(),
    }
}

pub fn runtime_doctor_count_breakdown(counts: &BTreeMap<String, usize>) -> String {
    if counts.is_empty() {
        return "-".to_string();
    }
    counts
        .iter()
        .map(|(label, count)| format!("{label}={count}"))
        .collect::<Vec<_>>()
        .join(", ")
}
