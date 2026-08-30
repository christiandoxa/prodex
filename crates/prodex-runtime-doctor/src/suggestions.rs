use super::*;

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorPolicySettingSuggestion {
    pub section: String,
    pub key: String,
    pub current_value: u64,
    pub suggested_value: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct RuntimeDoctorPolicySuggestion {
    pub id: String,
    pub title: String,
    pub severity: String,
    pub reason: String,
    pub markers: Vec<String>,
    pub settings: Vec<RuntimeDoctorPolicySettingSuggestion>,
    pub snippet: String,
}

#[cfg(not(feature = "mojo"))]
mod compatibility;
#[cfg(feature = "mojo")]
mod mojo;
#[cfg(feature = "mojo")]
pub(crate) use mojo::runtime_doctor_plan_input;

#[cfg(feature = "mojo")]
pub fn runtime_doctor_policy_suggestions(
    summary: &RuntimeDoctorSummary,
    snapshot: RuntimeDoctorTuningSnapshot,
) -> Vec<RuntimeDoctorPolicySuggestion> {
    mojo::policy_suggestions(summary, snapshot)
}

#[cfg(not(feature = "mojo"))]
pub fn runtime_doctor_policy_suggestions(
    summary: &RuntimeDoctorSummary,
    snapshot: RuntimeDoctorTuningSnapshot,
) -> Vec<RuntimeDoctorPolicySuggestion> {
    compatibility::runtime_doctor_policy_suggestions(summary, snapshot)
}

pub fn runtime_doctor_policy_suggestion_lines(
    suggestions: &[RuntimeDoctorPolicySuggestion],
) -> Vec<String> {
    let mut lines = vec!["Runtime Policy Suggestions".to_string()];
    if suggestions.is_empty() {
        lines.push("No policy.toml suggestion matched the sampled runtime markers.".to_string());
        return lines;
    }
    for suggestion in suggestions {
        lines.push(format!("- {}: {}", suggestion.title, suggestion.reason));
        lines.push("  policy.toml:".to_string());
        for line in suggestion.snippet.lines() {
            lines.push(format!("  {line}"));
        }
    }
    lines
}

#[cfg(test)]
#[path = "../tests/src/suggestions.rs"]
mod tests;
