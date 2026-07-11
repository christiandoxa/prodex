//! Runtime-log and audit emission for app-server broker preview sessions.

use crate::{
    runtime_proxy_log_field, runtime_proxy_log_to_path, runtime_proxy_structured_log_message,
};
use redaction::redaction_redact_secret_like_text;
use serde_json::Value;

pub(super) fn app_server_broker_audit_preview_summary_required_fields() -> [&'static str; 12] {
    [
        "mode",
        "line_count",
        "parsed_count",
        "error_count",
        "frame_kind_counts",
        "continuation_decision_counts",
        "policy_mode_counts",
        "commit_boundary_counts",
        "rotation_window_counts",
        "routing_hint_counts",
        "provider_switch_counts",
        "policy_flag_counts",
    ]
}

pub(super) fn app_server_broker_log_preview_event(
    log_path: &std::path::Path,
    line: usize,
    preview: &Value,
) {
    let rendered = if preview["preview"]["parse_ok"] == serde_json::Value::Bool(true) {
        let summary = &preview["preview"]["summary"];
        runtime_proxy_structured_log_message(
            "app_server_broker_observe",
            [
                runtime_proxy_log_field("line", line.to_string()),
                runtime_proxy_log_field("frame_id", app_server_broker_log_scalar(&summary["id"])),
                runtime_proxy_log_field("request_id", app_server_broker_log_scalar(&summary["id"])),
                runtime_proxy_log_field(
                    "frame_kind",
                    summary["frame_kind"].as_str().unwrap_or("unknown"),
                ),
                runtime_proxy_log_field(
                    "method_kind",
                    summary["method_kind"].as_str().unwrap_or("unknown"),
                ),
                runtime_proxy_log_field("valid_jsonrpc", summary["valid_jsonrpc"].to_string()),
                runtime_proxy_log_field("method", app_server_broker_log_scalar(&summary["method"])),
                runtime_proxy_log_field(
                    "lifecycle_stage",
                    summary["lifecycle_stage"].as_str().unwrap_or("-"),
                ),
                runtime_proxy_log_field(
                    "lifecycle_schema_file",
                    summary["lifecycle_schema_file"].as_str().unwrap_or("-"),
                ),
                runtime_proxy_log_field(
                    "continuation_decision",
                    summary["continuation_decision"].as_str().unwrap_or("fresh"),
                ),
                runtime_proxy_log_field(
                    "policy_mode",
                    summary["policy_hint"]["mode"]
                        .as_str()
                        .unwrap_or("fresh-selection-ok"),
                ),
                runtime_proxy_log_field(
                    "commit_boundary",
                    summary["policy_hint"]["commit_boundary"]
                        .as_str()
                        .unwrap_or("precommit"),
                ),
                runtime_proxy_log_field(
                    "rotation_window",
                    summary["policy_hint"]["rotation_window"]
                        .as_str()
                        .unwrap_or("closed"),
                ),
                runtime_proxy_log_field(
                    "routing_hint",
                    summary["policy_hint"]["routing_hint"]
                        .as_str()
                        .unwrap_or("fresh-select-ok"),
                ),
                runtime_proxy_log_field(
                    "preserved_owner_kind",
                    summary["policy_hint"]["preserved_owner_kind"]
                        .as_str()
                        .unwrap_or("-"),
                ),
                runtime_proxy_log_field(
                    "affinity_required",
                    summary["policy_hint"]["affinity_required"].to_string(),
                ),
                runtime_proxy_log_field(
                    "rotation_allowed",
                    summary["policy_hint"]["rotation_allowed"].to_string(),
                ),
                runtime_proxy_log_field(
                    "provider_switch_allowed",
                    summary["policy_hint"]["rotation_allowed"].to_string(),
                ),
                runtime_proxy_log_field(
                    "provider_switch_requires_override",
                    (!summary["policy_hint"]["rotation_allowed"]
                        .as_bool()
                        .unwrap_or(false))
                    .to_string(),
                ),
                runtime_proxy_log_field(
                    "turn_committed",
                    summary["policy_hint"]["turn_committed"].to_string(),
                ),
                runtime_proxy_log_field(
                    "preserves_owner",
                    summary["policy_hint"]["preserves_owner"].to_string(),
                ),
                runtime_proxy_log_field(
                    "owner_kind",
                    summary["continuation_affinity"]["owner_kind"]
                        .as_str()
                        .unwrap_or("none"),
                ),
                runtime_proxy_log_field(
                    "affinity_key_count",
                    summary["continuation_affinity"]["key_count"].to_string(),
                ),
                runtime_proxy_log_field(
                    "session_id",
                    app_server_broker_log_scalar(&summary["metadata"]["session_id"]),
                ),
                runtime_proxy_log_field(
                    "thread_id",
                    app_server_broker_log_scalar(&summary["metadata"]["thread_id"]),
                ),
                runtime_proxy_log_field(
                    "turn_id",
                    app_server_broker_log_scalar(&summary["metadata"]["turn_id"]),
                ),
                runtime_proxy_log_field(
                    "item_id",
                    app_server_broker_log_scalar(&summary["metadata"]["item_id"]),
                ),
                runtime_proxy_log_field(
                    "invalid_reason",
                    summary["invalid_reason"].as_str().unwrap_or("-"),
                ),
            ],
        )
    } else {
        runtime_proxy_structured_log_message(
            "app_server_broker_observe",
            [
                runtime_proxy_log_field("line", line.to_string()),
                runtime_proxy_log_field("parse_ok", "false"),
                runtime_proxy_log_field(
                    "error",
                    preview["preview"]["error"].as_str().unwrap_or("unknown"),
                ),
            ],
        )
    };
    runtime_proxy_log_to_path(log_path, &rendered);
}

fn app_server_broker_log_scalar(value: &Value) -> String {
    let rendered = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => "-".to_string(),
    };
    redaction_redact_secret_like_text(&rendered)
}

pub(super) fn app_server_broker_audit_preview_summary(mode: &str, summary: &Value) {
    let report = &summary["report"];
    let _ = crate::append_audit_event(
        "app_server_broker",
        "preview_session",
        "observed",
        serde_json::json!({
            "mode": mode,
            "line_count": report["line_count"],
            "parsed_count": report["parsed_count"],
            "error_count": report["error_count"],
            "frame_kind_counts": report["frame_kind_counts"],
            "continuation_decision_counts": report["continuation_decision_counts"],
            "policy_mode_counts": report["policy_mode_counts"],
            "commit_boundary_counts": report["commit_boundary_counts"],
            "rotation_window_counts": report["rotation_window_counts"],
            "routing_hint_counts": report["routing_hint_counts"],
            "provider_switch_counts": {
                "allowed": report["policy_flag_counts"]["rotation_allowed"],
                "requires_override": report["policy_flag_counts"]["affinity_required"],
            },
            "policy_flag_counts": report["policy_flag_counts"],
        }),
    );
}

pub(super) fn app_server_broker_log_preview_summary(log_path: &std::path::Path, summary: &Value) {
    let report = &summary["report"];
    runtime_proxy_log_to_path(
        log_path,
        &runtime_proxy_structured_log_message(
            "app_server_broker_summary",
            [
                runtime_proxy_log_field(
                    "line_count",
                    report["line_count"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "parsed_count",
                    report["parsed_count"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "error_count",
                    report["error_count"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "fresh_selection_ok_count",
                    report["policy_mode_counts"]["fresh-selection-ok"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "preserve_session_affinity_count",
                    report["policy_mode_counts"]["preserve-session-affinity"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "preserve_thread_affinity_count",
                    report["policy_mode_counts"]["preserve-thread-affinity"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "preserve_turn_affinity_count",
                    report["policy_mode_counts"]["preserve-turn-affinity"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "precommit_count",
                    report["commit_boundary_counts"]["precommit"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "turn_committed_count",
                    report["commit_boundary_counts"]["turn-committed"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "rotation_open_count",
                    report["rotation_window_counts"]["open"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "rotation_closed_count",
                    report["rotation_window_counts"]["closed"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "fresh_select_ok_count",
                    report["routing_hint_counts"]["fresh-select-ok"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "preserve_session_owner_count",
                    report["routing_hint_counts"]["preserve-session-owner"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "preserve_thread_owner_count",
                    report["routing_hint_counts"]["preserve-thread-owner"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "preserve_turn_owner_count",
                    report["routing_hint_counts"]["preserve-turn-owner"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "affinity_required_count",
                    report["policy_flag_counts"]["affinity_required"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "rotation_allowed_count",
                    report["policy_flag_counts"]["rotation_allowed"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
                runtime_proxy_log_field(
                    "preserves_owner_count",
                    report["policy_flag_counts"]["preserves_owner"]
                        .as_u64()
                        .unwrap_or_default()
                        .to_string(),
                ),
            ],
        ),
    );
}
