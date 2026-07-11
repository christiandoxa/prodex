use terminal_ui::FieldRowsBuilder;

use crate::{RuntimeDoctorIncidentExplanation, RuntimeDoctorSummary, diagnosis};

fn runtime_doctor_incident_explainer_text(incident: &RuntimeDoctorIncidentExplanation) -> String {
    let evidence = if incident.evidence.is_empty() {
        "-".to_string()
    } else {
        incident.evidence.join(", ")
    };
    format!(
        "{} Evidence: {evidence}. Next: {}",
        incident.cause, incident.next_action
    )
}

pub(super) fn runtime_doctor_push_incident_explainer_rows(
    fields: &mut FieldRowsBuilder,
    summary: &RuntimeDoctorSummary,
) {
    let incidents = diagnosis::runtime_doctor_incident_explainer(summary);
    if incidents.is_empty() {
        fields.push("Incident explainer", "-");
        return;
    }
    for (index, incident) in incidents.iter().take(6).enumerate() {
        fields.push(
            format!("Incident {}", index + 1),
            runtime_doctor_incident_explainer_text(incident),
        );
    }
    let hidden = incidents.len().saturating_sub(6);
    if hidden > 0 {
        fields.push("Incident hidden", hidden.to_string());
    }
}
