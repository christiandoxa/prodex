use std::collections::BTreeMap;

use crate::RuntimeDoctorSummary;

use super::request_timeline::runtime_doctor_request_timeline_detail;

#[cfg(test)]
fn runtime_doctor_selection_bucket(marker: &str) -> Option<&'static str> {
    let semantics = prodex_mojo_core::rich::runtime_doctor_marker_semantics(marker)
        .expect("Mojo runtime-doctor marker semantics returned invalid output");
    match semantics.selection_bucket {
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_SELECTION_PICKED => Some("picked"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_SELECTION_KEPT => Some("kept"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_SELECTION_SKIPPED => Some("skipped"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_SELECTION_BLOCKED => Some("blocked"),
        prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_SELECTION_NONE => None,
        _ => unreachable!("validated Mojo selection bucket"),
    }
}

pub(super) fn runtime_doctor_record_selection_summary(
    summary: &mut RuntimeDoctorSummary,
    marker: &str,
    fields: &BTreeMap<String, String>,
) {
    use prodex_mojo_core::rich::{RuntimeDoctorSelectionDestination, RuntimeDoctorSelectionReason};

    // Field presence, not nonempty values, determines rejection-reason priority.
    let reason_fields =
        ["reason", "outcome", "event", "status"].map(|key| fields.get(key).map(String::as_str));
    let Some(plan) =
        prodex_mojo_core::rich::runtime_doctor_selection_record_plan(marker, reason_fields)
            .expect("Mojo runtime-doctor selection plan returned invalid output")
    else {
        return;
    };

    let (profiles, routes) = match plan.destination {
        RuntimeDoctorSelectionDestination::Selected => (
            &mut summary.selection_summary.selected_profiles,
            &mut summary.selection_summary.selected_routes,
        ),
        RuntimeDoctorSelectionDestination::Rejected => (
            &mut summary.selection_summary.rejected_profiles,
            &mut summary.selection_summary.rejected_routes,
        ),
    };
    if let Some(profile) = fields.get("profile").filter(|value| !value.is_empty()) {
        *profiles.entry(profile.clone()).or_insert(0) += 1;
    }
    if let Some(route) = fields.get("route").filter(|value| !value.is_empty()) {
        *routes.entry(route.clone()).or_insert(0) += 1;
    }

    if let Some(reason_source) = plan.reason {
        let reason = match reason_source {
            RuntimeDoctorSelectionReason::Field(index) => reason_fields
                .get(index)
                .and_then(|value| *value)
                .expect("validated Mojo reason source must have a corresponding field"),
            RuntimeDoctorSelectionReason::Marker => marker,
        };
        *summary
            .selection_summary
            .rejection_reasons
            .entry(reason.to_string())
            .or_insert(0) += 1;
    }

    let detail = runtime_doctor_request_timeline_detail(fields);
    summary.selection_summary.latest_decision = Some(if detail.is_empty() {
        marker.to_string()
    } else {
        format!("{marker} {detail}")
    });
}

#[cfg(all(test, feature = "runtime-log-mojo"))]
mod expected_marker_buckets {
    use super::runtime_doctor_selection_bucket;

    use super::runtime_doctor_record_selection_summary;
    use crate::RuntimeDoctorSummary;
    use std::collections::BTreeMap;

    #[test]
    fn selection_summary_counts_and_rejection_reason_priority_are_mojo_owned() {
        let mut summary = RuntimeDoctorSummary::default();
        runtime_doctor_record_selection_summary(
            &mut summary,
            "selection_pick",
            &BTreeMap::from([
                ("profile".to_string(), "alpha".to_string()),
                ("route".to_string(), "responses".to_string()),
            ]),
        );
        runtime_doctor_record_selection_summary(
            &mut summary,
            "selection_keep_current",
            &BTreeMap::from([("profile".to_string(), "alpha".to_string())]),
        );
        runtime_doctor_record_selection_summary(
            &mut summary,
            "selection_skip_current",
            &BTreeMap::from([
                ("profile".to_string(), "beta".to_string()),
                ("reason".to_string(), "".to_string()),
                ("outcome".to_string(), "not chosen".to_string()),
            ]),
        );
        runtime_doctor_record_selection_summary(
            &mut summary,
            "selection_skip_affinity",
            &BTreeMap::from([
                ("route".to_string(), "compact".to_string()),
                ("outcome".to_string(), "filtered".to_string()),
                ("event".to_string(), "wrong_event".to_string()),
            ]),
        );
        runtime_doctor_record_selection_summary(
            &mut summary,
            "local_selection_blocked",
            &BTreeMap::from([
                ("event".to_string(), "auth".to_string()),
                ("status".to_string(), "denied".to_string()),
            ]),
        );
        runtime_doctor_record_selection_summary(
            &mut summary,
            "precommit_budget_exhausted",
            &BTreeMap::from([("status".to_string(), "maxed".to_string())]),
        );
        runtime_doctor_record_selection_summary(
            &mut summary,
            "compact_candidate_exhausted",
            &BTreeMap::new(),
        );
        let previous_latest = summary.selection_summary.latest_decision.clone();
        runtime_doctor_record_selection_summary(
            &mut summary,
            "first_upstream_chunk",
            &BTreeMap::from([("reason".to_string(), "ignored".to_string())]),
        );

        assert_eq!(
            summary.selection_summary.selected_profiles.get("alpha"),
            Some(&2)
        );
        assert_eq!(
            summary.selection_summary.selected_routes.get("responses"),
            Some(&1)
        );
        assert_eq!(
            summary.selection_summary.rejected_profiles.get("beta"),
            Some(&1)
        );
        assert_eq!(
            summary.selection_summary.rejected_routes.get("compact"),
            Some(&1)
        );
        for reason in [
            "",
            "filtered",
            "auth",
            "maxed",
            "compact_candidate_exhausted",
        ] {
            assert_eq!(
                summary.selection_summary.rejection_reasons.get(reason),
                Some(&1),
                "{reason:?}"
            );
        }
        assert_eq!(summary.selection_summary.rejection_reasons.len(), 5);
        assert_eq!(summary.selection_summary.latest_decision, previous_latest);
    }

    #[test]
    fn selection_bucket_matches_fixed_cases() {
        for (bucket, markers) in [
            ("picked", &["selection_pick"][..]),
            (
                "kept",
                &["selection_keep_affinity", "selection_keep_current"][..],
            ),
            (
                "skipped",
                &[
                    "selection_skip_current",
                    "selection_skip_affinity",
                    "selection_skip_sync_probe",
                ][..],
            ),
            (
                "blocked",
                &[
                    "local_selection_blocked",
                    "responses_pre_send_skip",
                    "websocket_pre_send_skip",
                    "quota_critical_floor_before_send",
                    "precommit_budget_exhausted",
                    "compact_precommit_budget_exhausted",
                    "compact_candidate_exhausted",
                ][..],
            ),
        ] {
            for marker in markers {
                assert_eq!(
                    runtime_doctor_selection_bucket(marker),
                    Some(bucket),
                    "{marker}"
                );
            }
        }
        for marker in ["selection_plan", "first_upstream_chunk", "unknown_marker"] {
            assert_eq!(runtime_doctor_selection_bucket(marker), None, "{marker}");
        }
    }
}
