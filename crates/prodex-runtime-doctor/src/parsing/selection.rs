use std::collections::BTreeMap;

use crate::RuntimeDoctorSummary;

use super::request_timeline::runtime_doctor_request_timeline_detail;

#[cfg(feature = "mojo")]
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

#[cfg(not(feature = "mojo"))]
fn runtime_doctor_selection_bucket(marker: &str) -> Option<&'static str> {
    runtime_doctor_selection_bucket_rust(marker)
}

#[cfg(any(not(feature = "mojo"), test))]
fn runtime_doctor_selection_bucket_rust(marker: &str) -> Option<&'static str> {
    match marker {
        "selection_pick" => Some("picked"),
        "selection_keep_affinity" | "selection_keep_current" => Some("kept"),
        "selection_skip_current" | "selection_skip_affinity" | "selection_skip_sync_probe" => {
            Some("skipped")
        }
        "local_selection_blocked"
        | "responses_pre_send_skip"
        | "websocket_pre_send_skip"
        | "quota_critical_floor_before_send"
        | "precommit_budget_exhausted"
        | "compact_precommit_budget_exhausted"
        | "compact_candidate_exhausted" => Some("blocked"),
        _ => None,
    }
}

pub(super) fn runtime_doctor_record_selection_summary(
    summary: &mut RuntimeDoctorSummary,
    marker: &str,
    fields: &BTreeMap<String, String>,
) {
    let Some(bucket) = runtime_doctor_selection_bucket(marker) else {
        return;
    };
    match bucket {
        "picked" => summary.selection_summary.picked += 1,
        "kept" => summary.selection_summary.kept += 1,
        "skipped" => summary.selection_summary.skipped += 1,
        "blocked" => summary.selection_summary.blocked += 1,
        _ => {}
    }

    let profile = fields.get("profile").filter(|value| !value.is_empty());
    let route = fields.get("route").filter(|value| !value.is_empty());
    if matches!(bucket, "picked" | "kept") {
        if let Some(profile) = profile {
            *summary
                .selection_summary
                .selected_profiles
                .entry(profile.clone())
                .or_insert(0) += 1;
        }
        if let Some(route) = route {
            *summary
                .selection_summary
                .selected_routes
                .entry(route.clone())
                .or_insert(0) += 1;
        }
    } else {
        if let Some(profile) = profile {
            *summary
                .selection_summary
                .rejected_profiles
                .entry(profile.clone())
                .or_insert(0) += 1;
        }
        if let Some(route) = route {
            *summary
                .selection_summary
                .rejected_routes
                .entry(route.clone())
                .or_insert(0) += 1;
        }
    }

    if matches!(bucket, "skipped" | "blocked") {
        let reason = fields
            .get("reason")
            .or_else(|| fields.get("outcome"))
            .or_else(|| fields.get("event"))
            .or_else(|| fields.get("status"))
            .map(String::as_str)
            .unwrap_or(marker);
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

#[cfg(all(test, feature = "mojo"))]
mod mojo_semantics_parity_tests {
    use super::*;
    use crate::markers::RuntimeDoctorMarker;

    #[test]
    fn selection_bucket_matches_rust_oracle_for_every_marker() {
        for marker in RuntimeDoctorMarker::ALL {
            let marker = marker.as_str();
            assert_eq!(
                runtime_doctor_selection_bucket(marker),
                runtime_doctor_selection_bucket_rust(marker),
                "{marker}"
            );
        }
        assert_eq!(runtime_doctor_selection_bucket("unknown_marker"), None);
    }
}
