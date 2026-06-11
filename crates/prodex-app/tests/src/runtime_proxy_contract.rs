use std::path::{Path, PathBuf};

use super::*;

#[test]
fn runtime_proxy_contract_summary_maps_to_eight_facets() {
    let summary = prodex_app_reports::format_runtime_proxy_contract_summary();
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

    let json = prodex_app_reports::runtime_proxy_contract_json_value();
    let object = json.as_object().expect("contract JSON should be an object");
    assert_eq!(object.len(), 8);
    assert!(object.values().all(|value| value == true));
}

#[test]
fn runtime_proxy_contract_has_source_level_evidence() {
    let root = workspace_root();
    let contract_evidence = [
        ContractEvidence {
            name: "scoped_gateway",
            files: &["README.md", "docs/runtime-policy.md"],
            required: &[
                "Prodex stays a scoped Codex gateway, not a general-purpose LLM SDK.",
                "If you only use one Codex account and do not need quota rotation, you probably do not need `prodex`.",
            ],
        },
        ContractEvidence {
            name: "policy_visible_selection",
            files: &[
                "crates/prodex-app/src/app_commands/info_handler.rs",
                "crates/prodex-app/src/app_commands/doctor.rs",
                "crates/prodex-app/src/runtime_proxy/selection.rs",
                "crates/prodex-app/src/runtime_proxy/selection/next.rs",
                "crates/prodex-app/src/runtime_proxy/selection_plan.rs",
            ],
            required: &["Runtime proxy contract", "selection_plan", "selection_pick"],
        },
        ContractEvidence {
            name: "bounded_precommit_retry",
            files: &[
                "crates/prodex-app/src/runtime_proxy/responses.rs",
                "crates/prodex-app/src/runtime_proxy/standard/compact.rs",
                "crates/prodex-app/src/runtime_proxy/websocket_message/loop_control.rs",
            ],
            required: &[
                "runtime_proxy_precommit_budget_exhausted_for_route",
                "selection_attempts = selection_attempts.saturating_add(1)",
                "precommit_budget_exhausted",
            ],
        },
        ContractEvidence {
            name: "cheap_hot_path",
            files: &[
                "crates/prodex-app/src/runtime_background/scheduled_save.rs",
                "crates/prodex-app/src/runtime_background/scheduled_save/queues.rs",
                "README.md",
            ],
            required: &[
                "state_save_queued",
                "state_save_queue_backpressure",
                "Runtime hot paths must avoid broad disk reads, quota probes, or blocking state saves.",
            ],
        },
        ContractEvidence {
            name: "quota_transport_split",
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
                "crates/prodex-app/src/runtime_background/scheduled_save.rs",
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
            name: "connection_reuse",
            files: &[
                "crates/prodex-app/src/runtime_proxy/websocket/response_tracking/session.rs",
                "crates/prodex-app/src/runtime_proxy/websocket_message/continuation_handling.rs",
                "crates/prodex-runtime-proxy/src/selection_policy.rs",
            ],
            required: &[
                "websocket_reuse_start",
                "reuse_existing_session",
                "runtime_websocket_reuse_watchdog_previous_response_fresh_fallback_allowed",
            ],
        },
        ContractEvidence {
            name: "profile_isolated_secrets",
            files: &[
                "crates/prodex-secret-store/src/lib.rs",
                "crates/prodex-secret-store/src/model.rs",
                "crates/prodex-redaction/src/lib.rs",
                "crates/prodex-app/src/profile_commands/import_export/import.rs",
            ],
            required: &[
                "SecretLocation",
                "redaction_redacted_body_snippet",
                "audit_log_event_best_effort",
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
