use super::*;
use std::collections::BTreeMap;

fn runtime_doctor_broker_artifact_fields(line: &str) -> BTreeMap<String, String> {
    line.split_whitespace()
        .filter_map(|token| token.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

pub(super) fn runtime_doctor_broker_issue_diagnosis(
    summary: &RuntimeDoctorSummary,
) -> Option<String> {
    let mut stale_lease_diagnosis = None;
    for line in &summary.runtime_broker_identities {
        let fields = runtime_doctor_broker_artifact_fields(line);
        let broker_key = fields
            .get("broker_key")
            .map(String::as_str)
            .unwrap_or("unknown");
        let pid = fields.get("pid").map(String::as_str).unwrap_or("-");
        let listen_addr = fields.get("listen_addr").map(String::as_str).unwrap_or("-");
        let stale_leases = fields
            .get("stale_leases")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        match fields
            .get("status")
            .map(String::as_str)
            .unwrap_or("unknown")
        {
            "dead_pid" => {
                return Some(format!(
                    "Runtime broker registry {broker_key} points to dead pid {pid} at {listen_addr}; run `prodex cleanup` or restart `prodex run` so a fresh broker registry is written."
                ));
            }
            "health_timeout" => {
                return Some(format!(
                    "Runtime broker {broker_key} pid {pid} at {listen_addr} is alive but the health probe timed out; inspect local listener state and restart active prodex/codex sessions if it does not recover."
                ));
            }
            "health_unreachable" => {
                return Some(format!(
                    "Runtime broker {broker_key} pid {pid} at {listen_addr} is alive but the health probe is unreachable; inspect the local listener and restart active prodex/codex sessions if needed."
                ));
            }
            "binary_mismatch" => {
                let version = fields.get("version").map(String::as_str).unwrap_or("-");
                let path = fields.get("path").map(String::as_str).unwrap_or("-");
                let mismatch = fields
                    .get("mismatch")
                    .map(String::as_str)
                    .unwrap_or("unknown");
                return Some(format!(
                    "Runtime broker {broker_key} pid {pid} is running a different prodex binary ({mismatch}, observed version {version}, path {path}); restart active prodex/codex sessions so the current binary is loaded."
                ));
            }
            _ if stale_leases > 0 => {
                stale_lease_diagnosis = Some(format!(
                    "Runtime broker {broker_key} still has {stale_leases} stale lease file(s); run `prodex cleanup` after old terminals using that broker have exited."
                ));
            }
            _ => {}
        }
    }
    stale_lease_diagnosis
}
