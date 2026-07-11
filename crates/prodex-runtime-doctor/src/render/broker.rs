use std::collections::BTreeMap;

use super::*;

#[derive(serde::Serialize)]
pub(super) struct RuntimeDoctorBrokerArtifactJsonView {
    #[serde(flatten)]
    fields: BTreeMap<String, serde_json::Value>,
}

fn runtime_doctor_parse_broker_artifact(line: &str) -> BTreeMap<String, String> {
    line.split_whitespace()
        .filter_map(|token| token.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

pub(super) fn runtime_doctor_runtime_broker_issue_lines(
    summary: &RuntimeDoctorSummary,
) -> Vec<String> {
    summary
        .runtime_broker_identities
        .iter()
        .filter_map(|line| {
            let artifact = runtime_doctor_parse_broker_artifact(line);
            let broker_key = artifact.get("broker_key")?;
            let status = artifact.get("status").map(String::as_str).unwrap_or("unknown");
            let pid = artifact.get("pid").map(String::as_str).unwrap_or("-");
            let stale_leases = artifact
                .get("stale_leases")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            let issue = match status {
                "dead_pid" => Some(format!(
                    "{broker_key}: registry points to dead pid {pid}; run prodex cleanup or restart prodex run"
                )),
                "health_timeout" => Some(format!(
                    "{broker_key}: pid {pid} health probe timed out; check local listener then restart prodex run if it stays stuck"
                )),
                "health_unreachable" => Some(format!(
                    "{broker_key}: pid {pid} health probe unreachable; check local listener then restart prodex run if needed"
                )),
                "binary_mismatch" => Some(format!(
                    "{broker_key}: pid {pid} runs different prodex binary; restart active prodex/codex sessions"
                )),
                _ => None,
            };
            match (issue, stale_leases) {
                (Some(issue), leases) if leases > 0 => Some(format!(
                    "{issue}; {leases} stale lease(s) remain, run prodex cleanup after old terminals exit"
                )),
                (Some(issue), _) => Some(issue),
                (None, leases) if leases > 0 => Some(format!(
                    "{broker_key}: {leases} stale lease(s) remain; run prodex cleanup after old terminals exit"
                )),
                (None, _) => None,
            }
        })
        .collect()
}

pub(super) fn runtime_doctor_broker_artifact_json_view(
    line: &str,
) -> RuntimeDoctorBrokerArtifactJsonView {
    let artifact = runtime_doctor_parse_broker_artifact(line);
    let mut fields = BTreeMap::new();
    for (key, value) in artifact {
        let json_value = if key == "stale_leases" {
            value
                .parse::<usize>()
                .map(serde_json::Value::from)
                .unwrap_or_else(|_| serde_json::Value::String(value))
        } else {
            serde_json::Value::String(value)
        };
        fields.insert(key, json_value);
    }
    RuntimeDoctorBrokerArtifactJsonView { fields }
}
