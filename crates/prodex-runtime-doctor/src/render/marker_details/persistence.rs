use super::*;

pub(super) fn local_writer_error(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    fields
        .push(
            "State save backlog",
            runtime_doctor_format_option(summary.state_save_queue_backlog),
        )
        .push(
            "State save lag",
            runtime_doctor_format_option(summary.state_save_lag_ms),
        )
        .push(
            "Cont journal backlog",
            runtime_doctor_format_option(summary.continuation_journal_save_backlog),
        )
        .push(
            "Cont journal lag",
            runtime_doctor_format_option(summary.continuation_journal_save_lag_ms),
        )
        .push(
            "Probe backlog",
            runtime_doctor_format_option(summary.profile_probe_refresh_backlog),
        )
        .push(
            "Probe lag",
            runtime_doctor_format_option(summary.profile_probe_refresh_lag_ms),
        );
}

pub(super) fn state_backpressure(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let marker = "state_save_queue_backpressure";
    if diagnosis::runtime_doctor_marker_count(summary, marker) == 0 {
        return;
    }
    fields
        .push(
            "State pressure reason",
            marker_field(summary, marker, "reason"),
        )
        .push(
            "State pressure backlog",
            marker_field(summary, marker, "backlog"),
        )
        .push(
            "Persistence next step",
            diagnosis::runtime_doctor_persistence_backpressure_next_step(summary),
        );
}

pub(super) fn journal_backpressure(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let marker = "continuation_journal_queue_backpressure";
    if diagnosis::runtime_doctor_marker_count(summary, marker) == 0 {
        return;
    }
    fields
        .push(
            "Cont journal pressure reason",
            marker_field(summary, marker, "reason"),
        )
        .push(
            "Cont journal pressure backlog",
            marker_field(summary, marker, "backlog"),
        );
}

pub(super) fn probe_backpressure(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    let marker = "profile_probe_refresh_backpressure";
    if diagnosis::runtime_doctor_marker_count(summary, marker) == 0 {
        return;
    }
    fields
        .push(
            "Probe pressure profile",
            marker_field(summary, marker, "profile"),
        )
        .push(
            "Probe pressure backlog",
            marker_field(summary, marker, "backlog"),
        )
        .push(
            "Probe next step",
            diagnosis::runtime_doctor_probe_refresh_backpressure_next_step(summary),
        );
}

pub(super) fn startup_audit(fields: &mut FieldRowsBuilder, summary: &RuntimeDoctorSummary) {
    fields.push("Startup pressure", summary.startup_audit_pressure.clone());
}
