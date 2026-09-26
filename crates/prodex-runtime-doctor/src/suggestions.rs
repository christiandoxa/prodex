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

mod mojo;
pub(crate) use mojo::runtime_doctor_plan_input;

pub fn runtime_doctor_policy_suggestions(
    summary: &RuntimeDoctorSummary,
    snapshot: RuntimeDoctorTuningSnapshot,
) -> Vec<RuntimeDoctorPolicySuggestion> {
    mojo::policy_suggestions(summary, snapshot)
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

#[cfg(all(test, feature = "runtime-log-mojo"))]
#[path = "../tests/src/suggestions.rs"]
mod tests;

#[cfg(test)]
mod expected_tests {
    use super::*;

    #[test]
    fn policy_suggestion_is_available_without_optional_features() {
        let mut summary = RuntimeDoctorSummary::default();
        summary
            .marker_counts
            .insert("runtime_proxy_lane_limit_reached".to_string(), 2);
        summary.marker_last_fields.insert(
            "runtime_proxy_lane_limit_reached".to_string(),
            [("lane".to_string(), "compact".to_string())].into(),
        );
        let suggestions =
            runtime_doctor_policy_suggestions(&summary, RuntimeDoctorTuningSnapshot::default());
        assert_eq!(suggestions[0].id, "lane_pressure");
        assert_eq!(suggestions[0].settings[0].key, "compact_active_limit");
        assert_eq!(suggestions[0].settings[0].suggested_value, 3);
    }
}
