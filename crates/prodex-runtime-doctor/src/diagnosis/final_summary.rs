use super::route_health::runtime_doctor_route_health;
use super::*;

mod compact;
mod continuations;
mod log_summary;
mod mojo;

pub use log_summary::runtime_doctor_finalize_log_summary;

pub fn runtime_doctor_top_facet(summary: &RuntimeDoctorSummary, facet: &str) -> Option<String> {
    let counts = summary.facet_counts.get(facet)?;
    let selection = prodex_mojo_core::rich::runtime_doctor_top_facet_selection(
        counts.iter().map(|(value, count)| (value.as_str(), *count)),
    )
    .expect("Mojo runtime-doctor facet reducer returned invalid output")?;
    let (value, count) = counts
        .iter()
        .nth(selection.index)
        .expect("Mojo runtime-doctor facet index is in range");
    debug_assert_eq!(*count, selection.count);
    Some(format!("{value} ({count})"))
}

pub fn runtime_doctor_finalize_summary(summary: &mut RuntimeDoctorSummary) {
    let plan = mojo::runtime_doctor_summary_plan(summary);
    summary.selection_pressure = mojo::pressure_label(plan.selection_pressure).to_string();
    summary.transport_pressure = mojo::pressure_label(plan.transport_pressure).to_string();
    summary.persistence_pressure = mojo::pressure_label(plan.persistence_pressure).to_string();
    summary.quota_freshness_pressure =
        mojo::pressure_label(plan.quota_freshness_pressure).to_string();
    summary.startup_audit_pressure = mojo::pressure_label(plan.startup_audit_pressure).to_string();
    summary.route_health = runtime_doctor_route_health(summary);
    if summary.diagnosis.is_empty() {
        summary.diagnosis = mojo::runtime_doctor_default_diagnosis(summary, &plan);
    }
}

pub fn runtime_doctor_append_pointer_note(
    summary: &mut RuntimeDoctorSummary,
    pointer_note: Option<&str>,
) {
    if let Some(note) = pointer_note
        && !summary.diagnosis.contains(note)
    {
        summary.diagnosis = format!("{} {}", summary.diagnosis, note);
    }
}
