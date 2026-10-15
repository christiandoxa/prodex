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
    // Rust only locates borrowed log fields. Mojo parses each decimal value
    // and selects the first valid source in every priority-ordered group.
    let metric_fields = [
        runtime_doctor_marker_last_field(summary, "state_save_queue_backpressure", "backlog"),
        runtime_doctor_marker_last_field(summary, "state_save_queued", "backlog"),
        runtime_doctor_marker_last_field(summary, "state_save_ok", "lag_ms"),
        runtime_doctor_marker_last_field(summary, "state_save_skipped", "lag_ms"),
        runtime_doctor_marker_last_field(summary, "state_save_error", "lag_ms"),
        runtime_doctor_marker_last_field(
            summary,
            "continuation_journal_queue_backpressure",
            "backlog",
        ),
        runtime_doctor_marker_last_field(summary, "continuation_journal_save_queued", "backlog"),
        runtime_doctor_marker_last_field(summary, "continuation_journal_save_ok", "lag_ms"),
        runtime_doctor_marker_last_field(summary, "continuation_journal_save_error", "lag_ms"),
        runtime_doctor_marker_last_field(summary, "profile_probe_refresh_backpressure", "backlog"),
        runtime_doctor_marker_last_field(summary, "profile_probe_refresh_queued", "backlog"),
        runtime_doctor_marker_last_field(summary, "profile_probe_refresh_ok", "lag_ms"),
        runtime_doctor_marker_last_field(summary, "profile_probe_refresh_error", "lag_ms"),
    ];
    let [
        state_backlog,
        state_lag,
        journal_backlog,
        journal_lag,
        probe_backlog,
        probe_lag,
    ] = prodex_mojo_core::rich::runtime_doctor_log_metric_plan(metric_fields)
        .expect("Mojo runtime-doctor log metric plan returned invalid output");
    summary.state_save_queue_backlog =
        state_backlog.map(|value| usize::try_from(value).expect("Mojo validated host usize range"));
    summary.state_save_lag_ms = state_lag;
    summary.continuation_journal_save_backlog = journal_backlog
        .map(|value| usize::try_from(value).expect("Mojo validated host usize range"));
    summary.continuation_journal_save_lag_ms = journal_lag;
    summary.profile_probe_refresh_backlog =
        probe_backlog.map(|value| usize::try_from(value).expect("Mojo validated host usize range"));
    summary.profile_probe_refresh_lag_ms = probe_lag;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn marker_field(summary: &mut RuntimeDoctorSummary, marker: &str, field: &str, value: &str) {
        summary
            .marker_last_fields
            .entry(marker.to_owned())
            .or_default()
            .insert(field.to_owned(), value.to_owned());
    }

    #[test]
    fn final_log_summary_mojo_decodes_and_selects_metric_sources() {
        let mut summary = RuntimeDoctorSummary::default();
        marker_field(
            &mut summary,
            "state_save_queue_backpressure",
            "backlog",
            "-4",
        );
        marker_field(&mut summary, "state_save_queued", "backlog", "+0007");
        marker_field(
            &mut summary,
            "state_save_ok",
            "lag_ms",
            "18446744073709551616",
        );
        marker_field(&mut summary, "state_save_skipped", "lag_ms", "17");
        marker_field(&mut summary, "state_save_error", "lag_ms", "19");
        marker_field(
            &mut summary,
            "continuation_journal_queue_backpressure",
            "backlog",
            "0",
        );
        marker_field(
            &mut summary,
            "continuation_journal_save_queued",
            "backlog",
            "99",
        );
        marker_field(&mut summary, "continuation_journal_save_ok", "lag_ms", " ");
        marker_field(
            &mut summary,
            "continuation_journal_save_error",
            "lag_ms",
            "18446744073709551615",
        );
        marker_field(
            &mut summary,
            "profile_probe_refresh_backpressure",
            "backlog",
            "🧭",
        );
        marker_field(&mut summary, "profile_probe_refresh_queued", "backlog", "3");
        marker_field(&mut summary, "profile_probe_refresh_ok", "lag_ms", "26");
        marker_field(&mut summary, "profile_probe_refresh_error", "lag_ms", "500");

        runtime_doctor_finalize_log_summary(&mut summary);
        assert_eq!(summary.state_save_queue_backlog, Some(7));
        assert_eq!(summary.state_save_lag_ms, Some(17));
        assert_eq!(summary.continuation_journal_save_backlog, Some(0));
        assert_eq!(summary.continuation_journal_save_lag_ms, Some(u64::MAX));
        assert_eq!(summary.profile_probe_refresh_backlog, Some(3));
        assert_eq!(summary.profile_probe_refresh_lag_ms, Some(26));
        assert!(summary.failure_class_counts.is_empty());
    }

    #[test]
    fn final_log_summary_mojo_keeps_missing_numeric_fields_absent() {
        let mut summary = RuntimeDoctorSummary::default();
        runtime_doctor_finalize_log_summary(&mut summary);
        assert_eq!(summary.state_save_queue_backlog, None);
        assert_eq!(summary.state_save_lag_ms, None);
        assert_eq!(summary.continuation_journal_save_backlog, None);
        assert_eq!(summary.continuation_journal_save_lag_ms, None);
        assert_eq!(summary.profile_probe_refresh_backlog, None);
        assert_eq!(summary.profile_probe_refresh_lag_ms, None);
    }
}
