use super::*;

#[test]
fn runtime_doctor_finalize_summary_uses_broker_artifact_diagnosis() {
    let mut summary = RuntimeDoctorSummary {
            pointer_exists: true,
            log_exists: true,
            line_count: 1,
            runtime_broker_identities: vec![
                "broker_key=broker-a pid=123 listen_addr=127.0.0.1:1234 status=dead_pid mismatch=none version=0.1.0 path=/opt/prodex sha256=abc123 source=registry stale_leases=2"
                    .to_string(),
            ],
            ..RuntimeDoctorSummary::default()
        };

    runtime_doctor_finalize_summary(&mut summary);

    assert!(summary.diagnosis.contains("broker-a") && summary.diagnosis.contains("dead pid 123"));
    assert!(summary.diagnosis.contains("prodex cleanup"));
}
