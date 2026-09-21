use std::path::{Path, PathBuf};

use super::*;

#[test]
fn runtime_proxy_contract_summary_maps_to_current_facets() {
    let summary = crate::reports::format_runtime_proxy_contract_summary();
    let facets = summary.split(", ").collect::<Vec<_>>();
    assert_eq!(
        facets,
        vec![
            "scoped gateway",
            "policy-visible selection",
            "bounded precommit retry",
            "cheap hot path",
            "quota/transport split",
            "structured observability",
            "connection reuse",
            "profile-isolated secrets",
        ]
    );
}

#[test]
fn runtime_proxy_contract_has_source_level_evidence() {
    let root = workspace_root();
    let contract_evidence = [
        ContractEvidence {
            name: "bounded_retry",
            files: &[
                "crates/prodex-app/src/runtime_proxy/precommit_loop.rs",
                "crates/prodex-app/src/runtime_proxy/responses.rs",
                "crates/prodex-app/src/runtime_proxy/websocket_message/loop_control.rs",
            ],
            required: &[
                "runtime_proxy_precommit_budget_exhausted_for_route",
                "selection_attempts",
                "precommit_budget_exhausted",
            ],
        },
        ContractEvidence {
            name: "quota_transport_separation",
            files: &[
                "crates/prodex-app/src/runtime_proxy/health_backoff.rs",
                "crates/prodex-runtime-proxy/src/error_policy.rs",
                "crates/prodex-runtime-proxy/src/quota.rs",
            ],
            required: &[
                "profile_transport_backoff",
                "insufficient_quota",
                "rate_limit_exceeded",
                "quota_critical_floor_before_send",
            ],
        },
        ContractEvidence {
            name: "structured_observability",
            files: &[
                "crates/prodex-app/src/runtime_proxy/precommit_loop.rs",
                "crates/prodex-app/src/runtime_proxy/health_performance.rs",
                "crates/prodex-runtime-doctor/tests/src/marker_guard.rs",
            ],
            required: &[
                "runtime_proxy_structured_log_message",
                "runtime_proxy_log_field",
                "runtime_doctor_marker_registry_covers_runtime_log_markers",
            ],
        },
        ContractEvidence {
            name: "profile_isolated_secrets",
            files: &[
                "crates/prodex-secret-store/src/locations.rs",
                "crates/prodex-redaction/src/lib.rs",
                "crates/prodex-app/src/profile_commands/import_export/import.rs",
            ],
            required: &[
                "SecretLocation",
                "redaction_redacted_body_snippet",
                "audit_log_event",
            ],
        },
    ];
    for evidence in contract_evidence {
        let combined = evidence
            .files
            .iter()
            .map(|path| read_workspace_file(&root, path))
            .collect::<String>();
        for required in evidence.required {
            assert!(
                combined.contains(required),
                "runtime proxy contract evidence '{}' missing '{}'",
                evidence.name,
                required
            );
        }
    }
}

struct ContractEvidence<'a> {
    name: &'a str,
    files: &'a [&'a str],
    required: &'a [&'a str],
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("prodex-app should live under crates/")
        .to_path_buf()
}

fn read_workspace_file(root: &Path, path: &str) -> String {
    let path = root.join(path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}
