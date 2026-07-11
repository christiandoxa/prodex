use chrono::{Local, TimeZone};
use terminal_ui::FieldRowsBuilder;

use crate::{
    RuntimeDoctorBindingSourceSummary, RuntimeDoctorBindingStateSummary, RuntimeDoctorSummary,
    diagnosis,
};

fn format_precise_reset_time(epoch: Option<i64>) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %:z").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn runtime_doctor_binding_source_text(source: &RuntimeDoctorBindingSourceSummary) -> String {
    let top_profile = source
        .profiles
        .first()
        .map(|profile| format!("{}={}", profile.profile, profile.total_bindings))
        .unwrap_or_else(|| "-".to_string());
    format!(
        "r={} s={} t={} sid={} total={} profiles={} top={}",
        source.response_bindings,
        source.session_bindings,
        source.turn_state_bindings,
        source.session_id_bindings,
        source.total_bindings,
        source.profile_count,
        top_profile
    )
}

fn runtime_doctor_binding_state_text(summary: &RuntimeDoctorBindingStateSummary) -> String {
    let missing = summary.state.missing_profile_bindings
        + summary.runtime_continuations.missing_profile_bindings
        + summary.continuation_journal.missing_profile_bindings
        + summary.merged_continuations.missing_profile_bindings;
    format!(
        "active={} profiles={} selected={} | state {} | runtime {} | journal {} | merged {} | missing={}",
        summary.active_profile.as_deref().unwrap_or("-"),
        summary.profile_count,
        summary.last_run_selected_profiles,
        runtime_doctor_binding_source_text(&summary.state),
        runtime_doctor_binding_source_text(&summary.runtime_continuations),
        runtime_doctor_binding_source_text(&summary.continuation_journal),
        runtime_doctor_binding_source_text(&summary.merged_continuations),
        missing
    )
}

fn runtime_doctor_request_timeline_text(summary: &RuntimeDoctorSummary) -> String {
    let Some(request_id) = summary.latest_request_id.as_deref() else {
        return "-".to_string();
    };
    if summary.latest_request_timeline.is_empty() {
        return format!("request={request_id}");
    }
    let events = summary
        .latest_request_timeline
        .iter()
        .map(|event| {
            let mut text = format!("{}:{}", event.phase, event.marker);
            if !event.detail.is_empty() {
                text.push(' ');
                text.push_str(&event.detail);
            }
            text
        })
        .collect::<Vec<_>>()
        .join(" -> ");
    format!("request={request_id} {events}")
}

pub(super) fn runtime_doctor_push_summary_tail_rows(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
    broker_issues: &[String],
    suspect_continuations: &str,
) {
    fields
        .push("Selection pressure", summary.selection_pressure.clone())
        .push("Transport pressure", summary.transport_pressure.clone())
        .push("Persistence pressure", summary.persistence_pressure.clone())
        .push("Quota freshness", summary.quota_freshness_pressure.clone())
        .push(
            "Failure classes",
            diagnosis::runtime_doctor_count_breakdown(&summary.failure_class_counts),
        )
        .push(
            "Persisted backoffs",
            format!(
                "retry={} transport={} circuits={}",
                summary.persisted_retry_backoffs,
                summary.persisted_transport_backoffs,
                summary.persisted_route_circuits
            ),
        )
        .push(
            "Persisted snapshots",
            format!(
                "{} total, {} stale",
                summary.persisted_usage_snapshots, summary.stale_persisted_usage_snapshots
            ),
        )
        .push(
            "Persisted continuations",
            format!(
                "responses={} sessions={} turns={} session_ids={} turn_coverage={}",
                summary.persisted_response_bindings,
                summary.persisted_session_bindings,
                summary.persisted_turn_state_bindings,
                summary.persisted_session_id_bindings,
                summary
                    .persisted_turn_state_coverage_percent
                    .map(|percent| format!("{percent}%"))
                    .unwrap_or_else(|| "-".to_string())
            ),
        )
        .push(
            "Binding state",
            runtime_doctor_binding_state_text(&summary.binding_state),
        )
        .push(
            "Continuation states",
            format!(
                "verified={} warm={} suspect={} dead={}",
                summary.persisted_verified_continuations,
                summary.persisted_warm_continuations,
                summary.persisted_suspect_continuations,
                summary.persisted_dead_continuations
            ),
        )
        .push(
            "Continuation journal",
            format!(
                "responses={} sessions={} turns={} session_ids={} saved_at={}",
                summary.persisted_continuation_journal_response_bindings,
                summary.persisted_continuation_journal_session_bindings,
                summary.persisted_continuation_journal_turn_state_bindings,
                summary.persisted_continuation_journal_session_id_bindings,
                summary
                    .continuation_journal_saved_at
                    .map(|epoch| format_precise_reset_time(Some(epoch)))
                    .unwrap_or_else(|| "-".to_string())
            ),
        )
        .push(
            "Recovered state",
            format!(
                "state={} continuations={} journal={} scores={} usage={} backoffs={} backups={}",
                summary.recovered_state_file,
                summary.recovered_continuations_file,
                summary.recovered_continuation_journal_file,
                summary.recovered_scores_file,
                summary.recovered_usage_snapshots_file,
                summary.recovered_backoffs_file,
                summary.last_good_backups_present
            ),
        )
        .push(
            "Degraded routes",
            if summary.degraded_routes.is_empty() {
                "-".to_string()
            } else {
                summary.degraded_routes.join(" | ")
            },
        )
        .push(
            "Orphan dirs",
            if summary.orphan_managed_dirs.is_empty() {
                "-".to_string()
            } else {
                summary.orphan_managed_dirs.join(", ")
            },
        )
        .push(
            "Prodex binaries",
            if summary.prodex_binary_identities.is_empty() {
                "-".to_string()
            } else {
                summary.prodex_binary_identities.join(" | ")
            },
        )
        .push(
            "Runtime brokers",
            if summary.runtime_broker_identities.is_empty() {
                "-".to_string()
            } else {
                summary.runtime_broker_identities.join(" | ")
            },
        )
        .push(
            "Broker issues",
            if broker_issues.is_empty() {
                "-".to_string()
            } else {
                broker_issues.join(" | ")
            },
        )
        .push(
            "Binary mismatch",
            format!(
                "installed={} broker={}",
                summary.prodex_binary_mismatch, summary.runtime_broker_mismatch
            ),
        )
        .push("Suspect continuations", suspect_continuations)
        .push(
            "Latest request timeline",
            runtime_doctor_request_timeline_text(summary),
        )
        .push(
            "Last marker",
            summary
                .last_marker_line
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        )
        .push("Diagnosis", summary.diagnosis.clone());
}
