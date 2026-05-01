use super::*;

#[cfg(test)]
pub(crate) use prodex_core::select_default_codex_home;
pub(crate) use prodex_core::{
    absolutize, default_codex_home, normalize_path_for_compare, same_path,
};

pub(crate) fn audit_log_event_best_effort(
    component: &str,
    action: &str,
    outcome: &str,
    details: serde_json::Value,
) {
    let _ = append_audit_event(component, action, outcome, details);
}
