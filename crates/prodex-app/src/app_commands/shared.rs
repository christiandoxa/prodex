use crate::audit_log::append_audit_event;

pub(crate) use prodex_core::{absolutize, default_codex_home};
#[cfg(test)]
pub(crate) use prodex_core::{same_path, select_default_codex_home};

pub(crate) fn audit_log_event_best_effort(
    component: &str,
    action: &str,
    outcome: &str,
    details: serde_json::Value,
) {
    let _ = append_audit_event(component, action, outcome, details);
}
