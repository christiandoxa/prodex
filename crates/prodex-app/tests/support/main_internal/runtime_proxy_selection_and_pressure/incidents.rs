use std::collections::BTreeMap;
use std::path::Path;

use super::*;

fn runtime_incident_replay_summary(tail: &str) -> RuntimeDoctorSummary {
    let mut summary = summarize_runtime_log_tail(tail.as_bytes());
    summary.pointer_exists = true;
    summary.log_exists = true;
    runtime_doctor_finalize_summary(&mut summary);
    summary
}

fn runtime_incident_replay_fields(summary: &RuntimeDoctorSummary) -> BTreeMap<String, String> {
    runtime_doctor_fields_for_summary(summary, Path::new("/tmp/prodex-runtime-latest.path"))
        .into_iter()
        .collect()
}

#[test]
fn runtime_incident_replay_classifies_previous_response_fallback_from_log_text() {
    let summary = runtime_incident_replay_summary(
        "[2026-04-11 12:00:00.000 +07:00] request=41 transport=http previous_response_fresh_fallback reason=previous_response_not_found request_shape=session_replayable outcome=session_replayable_recovery profile=second via=direct_current_profile_fallback\n",
    );
    let fields = runtime_incident_replay_fields(&summary);

    assert_eq!(
        runtime_doctor_marker_count(&summary, "previous_response_fresh_fallback"),
        1
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("previous_response_fresh_fallback")
            .and_then(|fields| fields.get("request_shape"))
            .map(String::as_str),
        Some("session_replayable")
    );
    assert_eq!(
        summary.diagnosis,
        "Legacy previous_response recovery marker was observed for session-scoped previous_response_id continuation, but current runtime should fail closed instead of treating this as recoverable. Latest reason: previous_response_not_found. Restart active prodex/codex sessions if this came from a live broker."
    );
    assert_eq!(
        fields.get("Legacy prev recovery").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        fields.get("Hot reason").map(String::as_str),
        Some("previous_response_not_found (1)")
    );
}

#[test]
fn runtime_incident_replay_classifies_compact_final_failure_from_log_text() {
    let summary = runtime_incident_replay_summary(
        "[2026-04-11 12:05:00.000 +07:00] request=52 transport=http compact_final_failure exit=quota_fallback_exhausted reason=quota attempts=2 elapsed_ms=186 pressure_mode=false last_failure=quota saw_inflight_saturation=false profile=main\n",
    );
    let fields = runtime_incident_replay_fields(&summary);

    assert_eq!(
        runtime_doctor_marker_count(&summary, "compact_final_failure"),
        1
    );
    assert_eq!(
        summary
            .marker_last_fields
            .get("compact_final_failure")
            .and_then(|fields| fields.get("exit"))
            .map(String::as_str),
        Some("quota_fallback_exhausted")
    );
    assert_eq!(
        summary.diagnosis,
        "Recent compact final failure exited via quota_fallback_exhausted with reason quota. Next step: Inspect compact budget and candidate-exhausted markers on profile main, then retry after compact quota refreshes or another profile becomes eligible."
    );
    assert_eq!(fields.get("Compact final").map(String::as_str), Some("1"));
    assert_eq!(
        fields.get("Compact exit").map(String::as_str),
        Some("quota_fallback_exhausted")
    );
    assert_eq!(
        fields.get("Compact reason").map(String::as_str),
        Some("quota")
    );
    assert_eq!(
        fields.get("Compact last fail").map(String::as_str),
        Some("quota")
    );
}

#[test]
fn runtime_incident_replay_surfaces_background_queue_markers_from_log_text() {
    let summary = runtime_incident_replay_summary(
        "[2026-04-11 12:10:00.000 +07:00] state_save_queue_backpressure revision=2 reason=session_id:main backlog=7\n\
[2026-04-11 12:10:00.001 +07:00] continuation_journal_queue_backpressure reason=session_id:main backlog=5\n\
[2026-04-11 12:10:00.002 +07:00] profile_probe_refresh_backpressure profile=second backlog=4\n\
[2026-04-11 12:10:00.003 +07:00] selection_skip_sync_probe route=responses reason=pressure_mode cold_start_jobs=3\n",
    );
    let fields = runtime_incident_replay_fields(&summary);

    assert_eq!(
        runtime_doctor_marker_count(&summary, "state_save_queue_backpressure"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "continuation_journal_queue_backpressure"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "profile_probe_refresh_backpressure"),
        1
    );
    assert_eq!(
        runtime_doctor_marker_count(&summary, "selection_skip_sync_probe"),
        1
    );
    assert_eq!(summary.state_save_queue_backlog, Some(7));
    assert_eq!(summary.continuation_journal_save_backlog, Some(5));
    assert_eq!(summary.profile_probe_refresh_backlog, Some(4));
    assert_eq!(summary.persistence_pressure, "elevated");
    assert_eq!(summary.quota_freshness_pressure, "stale_risk");
    assert_eq!(
        fields.get("Sync-probe deferred").map(String::as_str),
        Some("3 job(s)")
    );
    assert_eq!(
        fields.get("Probe pressure profile").map(String::as_str),
        Some("second")
    );
}
