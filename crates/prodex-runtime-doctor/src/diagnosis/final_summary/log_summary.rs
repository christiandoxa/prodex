use std::collections::BTreeMap;

use crate::RuntimeDoctorSummary;

use super::super::marker_accessors::*;
use super::runtime_doctor_top_facet;

fn runtime_doctor_failure_class_counts(summary: &RuntimeDoctorSummary) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for (marker, count) in &summary.marker_counts {
        if *count == 0 {
            continue;
        }
        let semantics = prodex_mojo_core::rich::runtime_doctor_marker_semantics(marker)
            .expect("Mojo runtime-doctor marker semantics returned invalid output");
        let label = match semantics.failure_class {
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_ADMISSION => "admission",
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_AUTH => "auth",
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_CONTINUATION => {
                "continuation"
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_PERSISTENCE => {
                "persistence"
            }
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_QUOTA => "quota",
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_TRANSPORT => "transport",
            prodex_mojo_core::rich::RUNTIME_DOCTOR_MARKER_FAILURE_CLASS_NONE => continue,
            _ => unreachable!("validated Mojo runtime-doctor failure class"),
        };
        *counts.entry(label).or_default() += *count;
    }
    counts
        .into_iter()
        .map(|(label, count)| (label.to_string(), count))
        .collect()
}

pub fn runtime_doctor_finalize_log_summary(summary: &mut RuntimeDoctorSummary) {
    summary.state_save_queue_backlog =
        runtime_doctor_marker_last_usize_field(summary, "state_save_queue_backpressure", "backlog")
            .or_else(|| {
                runtime_doctor_marker_last_usize_field(summary, "state_save_queued", "backlog")
            });
    summary.state_save_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "state_save_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(summary, "state_save_skipped", "lag_ms")
            })
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(summary, "state_save_error", "lag_ms")
            });
    summary.continuation_journal_save_backlog = runtime_doctor_marker_last_usize_field(
        summary,
        "continuation_journal_queue_backpressure",
        "backlog",
    )
    .or_else(|| {
        runtime_doctor_marker_last_usize_field(
            summary,
            "continuation_journal_save_queued",
            "backlog",
        )
    });
    summary.continuation_journal_save_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "continuation_journal_save_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(
                    summary,
                    "continuation_journal_save_error",
                    "lag_ms",
                )
            });
    summary.profile_probe_refresh_backlog = runtime_doctor_marker_last_usize_field(
        summary,
        "profile_probe_refresh_backpressure",
        "backlog",
    )
    .or_else(|| {
        runtime_doctor_marker_last_usize_field(summary, "profile_probe_refresh_queued", "backlog")
    });
    summary.profile_probe_refresh_lag_ms =
        runtime_doctor_marker_last_u64_field(summary, "profile_probe_refresh_ok", "lag_ms")
            .or_else(|| {
                runtime_doctor_marker_last_u64_field(
                    summary,
                    "profile_probe_refresh_error",
                    "lag_ms",
                )
            });
    // Count quota-floor pre-send skips via the reason facet so the doctor keeps
    // exposing the hardening signal even though the runtime logs it as a reason,
    // not as a standalone marker.
    let quota_floor_before_send_count =
        runtime_doctor_facet_count(summary, "reason", "quota_critical_floor_before_send");
    if quota_floor_before_send_count > 0 {
        *summary
            .marker_counts
            .entry("quota_critical_floor_before_send".to_string())
            .or_insert(0) += quota_floor_before_send_count;
    }
    summary.compat_warning_count = runtime_doctor_marker_count(summary, "compat_warning");
    summary.top_client_family = runtime_doctor_top_facet(summary, "family");
    summary.top_client = runtime_doctor_top_facet(summary, "client");
    summary.top_tool_surface = runtime_doctor_top_facet(summary, "tool_surface");
    summary.top_compat_warning = runtime_doctor_top_facet(summary, "warning");
    summary.failure_class_counts = runtime_doctor_failure_class_counts(summary);
}
