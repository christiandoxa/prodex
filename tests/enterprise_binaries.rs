use std::fs;
use std::process::Command;

fn bin(name: &str) -> String {
    std::env::var(format!("CARGO_BIN_EXE_{name}")).expect("cargo should expose binary path")
}

#[test]
fn gateway_binary_exposes_dedicated_help_and_version() {
    let help = Command::new(bin("prodex-gateway"))
        .arg("--help")
        .output()
        .expect("run prodex-gateway --help");
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    assert!(stdout.contains("Data-plane gateway entrypoint"));
    assert!(stdout.contains("composition root"));

    let version = Command::new(bin("prodex-gateway"))
        .arg("--version")
        .output()
        .expect("run prodex-gateway --version");
    assert!(version.status.success());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .starts_with("prodex-gateway ")
    );
}

#[test]
fn control_plane_binary_exposes_dedicated_help_and_version() {
    let help = Command::new(bin("prodex-control-plane"))
        .arg("--help")
        .output()
        .expect("run prodex-control-plane --help");
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    assert!(stdout.contains("Control-plane entrypoint"));
    assert!(stdout.contains("composition"));

    let version = Command::new(bin("prodex-control-plane"))
        .arg("--version")
        .output()
        .expect("run prodex-control-plane --version");
    assert!(version.status.success());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .starts_with("prodex-control-plane ")
    );
}

#[test]
fn enterprise_serve_commands_are_explicitly_gated_until_adapters_are_ready() {
    for name in ["prodex-gateway", "prodex-control-plane"] {
        let output = Command::new(bin(name))
            .arg("serve")
            .output()
            .expect("run gated serve command");
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("not wired yet")
        );
    }
}

#[test]
fn control_plane_binary_delivers_config_publication_event() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-delivery-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane root");
    fs::write(
        root.join("policy.toml"),
        "version = 1

[runtime_proxy]
worker_count = 3
",
    )
    .expect("write runtime policy");
    let event_path = root.join("publication-event.json");
    fs::write(
        &event_path,
        serde_json::json!({
            "tenant_id": "00000000-0000-7000-8000-000000000001",
            "activated_revision_id": "00000000-0000-7000-8000-000000000002",
            "previous_active_revision_id": "00000000-0000-7000-8000-000000000003",
            "last_known_good_revision_id": serde_json::Value::Null,
            "targets": ["gateway_cache_refresh", "runtime_policy_reload"]
        })
        .to_string(),
    )
    .expect("write publication event");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "deliver-config-publication",
            "--event",
            event_path.to_str().unwrap(),
            "--root",
            root.to_str().unwrap(),
        ])
        .output()
        .expect("run config publication delivery");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["gateway_cache_refreshed"], true);
    assert_eq!(stdout["runtime_policy_version"], 1);
    assert_eq!(
        stdout["delivery_metrics"][0]["target"],
        "gateway_cache_refresh"
    );
    assert_eq!(
        stdout["delivery_metrics"][1]["target"],
        "runtime_policy_reload"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_plans_config_publication() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane plan root");
    let request_path = root.join("publication-request.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000010",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "control_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000011",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write publication request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run config publication planning");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], true);
    assert_eq!(stdout["required_role"], "admin");
    assert_eq!(stdout["resource_kind"], "configuration");
    assert_eq!(stdout["resource_action"], "publish_revision");
    assert_eq!(
        stdout["audit_action"],
        "control_plane.configuration.publish"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn control_plane_binary_reports_config_publication_denial() {
    let root = std::env::temp_dir().join(format!(
        "prodex-control-plane-plan-denied-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).expect("create control-plane denied root");
    let request_path = root.join("publication-request-denied.json");
    fs::write(
        &request_path,
        serde_json::json!({
            "principal": {
                "id": "00000000-0000-7000-8000-000000000020",
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "kind": "user",
                "role": "admin",
                "credential_scope": "data_plane"
            },
            "occurred_at_unix_ms": 1700000000000u64,
            "current_revision_id": serde_json::Value::Null,
            "candidate": {
                "tenant_id": "00000000-0000-7000-8000-000000000001",
                "revision_id": "00000000-0000-7000-8000-000000000021",
                "published_at_unix_ms": 1700000000001u64,
                "payload": {
                    "policy": "v1"
                }
            }
        })
        .to_string(),
    )
    .expect("write denied publication request");

    let output = Command::new(bin("prodex-control-plane"))
        .args([
            "plan-config-publication",
            "--request",
            request_path.to_str().unwrap(),
        ])
        .output()
        .expect("run denied config publication planning");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(stdout["authorized"], false);
    assert_eq!(stdout["code"], "credential_scope_not_allowed");
    assert_eq!(stdout["audit_outcome"], "denied");

    let _ = fs::remove_dir_all(root);
}
