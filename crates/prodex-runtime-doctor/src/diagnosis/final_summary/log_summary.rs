use std::collections::BTreeMap;

use crate::RuntimeDoctorSummary;

use super::super::marker_accessors::*;
use super::runtime_doctor_top_facet;

fn runtime_doctor_marker_summary_counts(
    summary: &RuntimeDoctorSummary,
) -> prodex_mojo_core::rich::RuntimeDoctorMarkerSummaryCounts {
    prodex_mojo_core::rich::runtime_doctor_marker_summary_counts(
        summary
            .marker_counts
            .iter()
            .map(|(marker, count)| (marker.as_str(), *count)),
    )
    .expect("Mojo runtime-doctor summary counter returned invalid output")
}

fn runtime_doctor_failure_class_counts(
    counts: prodex_mojo_core::rich::RuntimeDoctorMarkerSummaryCounts,
) -> BTreeMap<String, usize> {
    [
        ("admission", counts.failure_admission),
        ("auth", counts.failure_auth),
        ("continuation", counts.failure_continuation),
        ("persistence", counts.failure_persistence),
        ("quota", counts.failure_quota),
        ("transport", counts.failure_transport),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
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
    let selection_counts = runtime_doctor_marker_summary_counts(summary);
    summary.selection_summary.picked = selection_counts.selection_picked;
    summary.selection_summary.kept = selection_counts.selection_kept;
    summary.selection_summary.skipped = selection_counts.selection_skipped;
    summary.selection_summary.blocked = selection_counts.selection_blocked;
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
    summary.failure_class_counts =
        runtime_doctor_failure_class_counts(runtime_doctor_marker_summary_counts(summary));
}
