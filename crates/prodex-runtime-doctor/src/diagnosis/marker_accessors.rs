use super::*;
use std::collections::BTreeMap;

pub fn runtime_doctor_marker_count(
    summary: &RuntimeDoctorSummary,
    marker: impl AsRef<str>,
) -> usize {
    let marker = marker.as_ref();
    summary.marker_counts.get(marker).copied().unwrap_or(0)
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
    marker: impl AsRef<str>,
    field: &str,
) -> Option<&'a str> {
    let marker = marker.as_ref();
    summary
        .marker_last_fields
        .get(marker)
        .and_then(|fields| fields.get(field))
        .map(String::as_str)
}

pub(super) fn runtime_doctor_marker_last_usize_field(
    summary: &RuntimeDoctorSummary,
    marker: impl AsRef<str>,
    field: &str,
) -> Option<usize> {
    prodex_mojo_core::rich::runtime_doctor_unsigned_field(runtime_doctor_marker_last_field(
        summary, marker, field,
    ))
    .expect("Mojo runtime-doctor numeric marker field returned invalid output")
}

pub(super) fn runtime_doctor_marker_scope(
    summary: &RuntimeDoctorSummary,
    marker: impl AsRef<str>,
    profile_field: &str,
    route_field: &str,
) -> Option<String> {
    let marker = marker.as_ref();
    let profile = runtime_doctor_marker_last_field(summary, marker, profile_field);
    let route = runtime_doctor_marker_last_field(summary, marker, route_field);
    prodex_mojo_core::rich::runtime_doctor_marker_scope_text(profile, route)
        .expect("Mojo runtime-doctor marker scope returned invalid output")
}

pub fn runtime_doctor_count_breakdown(counts: &BTreeMap<String, usize>) -> String {
    prodex_mojo_core::rich::runtime_doctor_count_breakdown_text(
        counts.iter().map(|(name, count)| (name.as_str(), *count)),
    )
    .expect("Mojo runtime-doctor count breakdown returned invalid output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_marker_value_is_parsed_only_by_mojo() {
        let mut summary = RuntimeDoctorSummary::default();
        assert_eq!(
            runtime_doctor_marker_last_usize_field(&summary, "queued", "backlog"),
            None,
        );
        for (input, expected) in [
            ("+00017", Some(17)),
            ("0", Some(0)),
            (" 7", None),
            ("-1", None),
            ("18446744073709551616", None),
        ] {
            summary.marker_last_fields.insert(
                "queued".to_string(),
                BTreeMap::from([("backlog".to_string(), input.to_string())]),
            );
            assert_eq!(
                runtime_doctor_marker_last_usize_field(&summary, "queued", "backlog"),
                expected,
                "{input}",
            );
        }
    }

    #[test]
    fn marker_scope_and_breakdown_are_mojo_rendered_from_real_summary_maps() {
        let mut summary = RuntimeDoctorSummary::default();
        summary.marker_last_fields.insert(
            "profile_health".to_string(),
            BTreeMap::from([
                ("profile".to_string(), "東京".to_string()),
                ("route".to_string(), "responses".to_string()),
            ]),
        );
        assert_eq!(
            runtime_doctor_marker_scope(&summary, "profile_health", "profile", "route"),
            Some("東京/responses".to_string()),
        );
        assert_eq!(
            runtime_doctor_marker_scope(&summary, "missing", "profile", "route"),
            None,
        );
        let counts = BTreeMap::from([("alpha".to_string(), 0), ("🧭".to_string(), usize::MAX)]);
        assert_eq!(
            runtime_doctor_count_breakdown(&counts),
            format!("alpha=0, 🧭={}", usize::MAX),
        );
    }
}
