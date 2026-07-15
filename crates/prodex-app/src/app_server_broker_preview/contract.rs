use super::super::app_server_broker_protocol::{
    app_server_broker_commit_boundaries, app_server_broker_continuation_decision_kinds,
    app_server_broker_lifecycle_methods, app_server_broker_policy_modes,
    app_server_broker_rotation_windows, app_server_broker_routing_hints,
};
use super::logging::app_server_broker_audit_preview_summary_required_fields;
use super::parse::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES;
use serde_json::Value;

pub(crate) fn app_server_broker_status_line() -> String {
    let contract = app_server_broker_contract_json();
    let status = contract
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let transport = contract
        .get("transport")
        .and_then(Value::as_array)
        .map(|transport| {
            transport
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|transport| !transport.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let default_mode = contract
        .get("default_mode")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!(
        "app-server broker: status={status}; transport={transport}; mode={default_mode}; passthrough-aware stdio preview is available; default Codex app-server passthrough remains active"
    )
}

pub(crate) fn app_server_broker_render_output(json: bool) -> anyhow::Result<String> {
    if json {
        Ok(serde_json::to_string_pretty(
            &app_server_broker_contract_json(),
        )?)
    } else {
        Ok(app_server_broker_status_line())
    }
}

pub(crate) fn app_server_broker_contract_json() -> serde_json::Value {
    serde_json::json!({
        "object": "app_server_broker.contract",
        "enabled_by_default": false,
        "status": "diagnostic-envelope-parsing",
        "transport": ["stdio-preview", "stdio-passthrough-preview", "stdio-validate", "stdio-validate-passthrough", "stdio-live"],
        "jsonrpc": "2.0",
        "wire_omits_jsonrpc_header": true,
        "default_mode": "direct-passthrough",
        "schema_validation": {
            "protocol_surface_fixture": true,
            "fixture_drift_tests": true,
            "helper_consistency_tests": true,
            "stream_helper_consistency_tests": true,
            "cross_surface_consistency_tests": true,
            "report_aggregation_consistency_tests": true,
            "metadata_surface_fixture": true,
            "metadata_drift_tests": true,
            "output_surface_fixture": true,
            "output_drift_tests": true,
            "runtime_log_surface_fixture": true,
            "runtime_log_drift_tests": true,
            "upstream_codex_schema_imported": true,
            "lifecycle_schema_hints": true,
            "lifecycle_response_schema_hints": true
        },
        "cli": {
            "json_contract": true,
            "experimental_stdio_preview": true,
            "experimental_stdio_live": true,
            "experimental_stdio_passthrough_preview": true,
            "experimental_stdio_validate": true,
            "experimental_stdio_validate_passthrough": true
        },
        "diagnostics": {
            "jsonrpc_envelope_validation": true,
            "max_preview_line_bytes": APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES,
            "frame_kinds": ["request", "notification", "response", "invalid"],
            "method_kinds": ["lifecycle", "other", "absent"],
            "invalid_reasons": [
                "non_jsonrpc_version",
                "batch_frame_unsupported",
                "non_object_frame",
                "non_scalar_id",
                "non_container_params",
                "non_object_error",
                "non_integer_error_code",
                "non_string_error_message",
                "non_string_method",
                "invalid_method_name",
                "result_with_error",
                "missing_response_id",
                "method_with_result_or_error",
                "missing_method_and_response_payload"
            ],
            "metadata_extraction": ["session_id", "thread_id", "turn_id", "item_id"],
            "replay_summary": true,
            "request_summary_json": true,
            "response_summary_json": true,
            "policy_hint": true,
            "commit_boundary_hint": true,
            "turn_committed_hint": true,
            "affinity_required_hint": true,
            "rotation_window_hint": true,
            "rotation_allowed_hint": true,
            "routing_hint": true,
            "provider_switch_policy_hint": true,
            "preserved_owner_kind_hint": true,
            "preserves_owner_hint": true,
            "policy_flag_counts": true,
            "audit_preview_summary": true,
            "stdio_preview_report": true,
            "stdio_preview_report_envelope": true,
            "stdio_preview_pretty_json": true,
            "stdio_preview_jsonl": true,
            "stdio_preview_session_summary": true,
            "stdio_passthrough_preview": true,
            "stdio_passthrough_preserves_input": true,
            "stdio_passthrough_empty_summary": true,
            "stdio_passthrough_write_failures_surface": true,
            "stdio_diagnostics_write_failures_surface": true,
            "stdio_validate_fail_closed": true,
            "stdio_validate_passthrough_fail_closed": true,
            "stdio_validate_passthrough_preserves_valid_input": true,
            "request_response_id_validation": true,
            "request_response_validation_reasons": [
                "request_missing_id",
                "response_missing_id",
                "response_without_request",
                "duplicate_pending_request_id",
                "pending_request_without_response",
                "lifecycle_response_missing_thread_id",
                "lifecycle_response_missing_thread_status",
                "lifecycle_response_invalid_thread_status",
                "lifecycle_response_missing_thread_context",
                "lifecycle_response_invalid_thread_context",
                "lifecycle_response_missing_thread_object_context",
                "lifecycle_response_missing_turn_id",
                "lifecycle_response_missing_turn_items",
                "lifecycle_response_missing_turn_status",
                "lifecycle_response_invalid_turn_status"
            ],
            "lifecycle_payload_validation": true,
            "lifecycle_payload_validation_reasons": [
                "lifecycle_missing_thread_id",
                "lifecycle_missing_thread_object_id",
                "lifecycle_missing_thread_context",
                "lifecycle_missing_thread_status",
                "lifecycle_invalid_thread_status",
                "lifecycle_missing_turn_input",
                "lifecycle_missing_turn_items",
                "lifecycle_invalid_turn_status",
                "lifecycle_missing_turn_status"
            ],
            "lifecycle_consistency_validation": true,
            "lifecycle_validation_reasons": [
                "turn_started_missing_turn_id",
                "turn_started_missing_thread_id",
                "turn_completed_missing_turn_id",
                "turn_completed_missing_thread_id",
                "turn_interrupt_missing_turn_id",
                "turn_interrupt_missing_thread_id",
                "turn_completed_without_turn_started",
                "turn_started_after_completed",
                "thread_active_turn_conflict",
                "turn_completed_not_active",
                "turn_interrupt_active_turn_conflict",
                "duplicate_turn_started",
                "duplicate_turn_completed"
            ],
            "invalid_frame_reasoning": "shape-based",
            "routing_changes": false
        },
        "lifecycle_methods": app_server_broker_lifecycle_methods(),
        "accepted_lifecycle_aliases": ["notifications/initialized", "turn/cancel"],
        "affinity": {
            "thread_session_owner_required": true,
            "continuation_affinity_wins": true,
            "rotate_only_before_turn_commit": true,
            "decision_kinds": app_server_broker_continuation_decision_kinds(),
            "policy_modes": app_server_broker_policy_modes(),
            "routing_hints": app_server_broker_routing_hints(),
            "commit_boundaries": app_server_broker_commit_boundaries(),
            "rotation_windows": app_server_broker_rotation_windows()
        },
        "audit": {
            "preview_summary": {
                "component": "app_server_broker",
                "action": "preview_session",
                "outcome": "observed",
                "modes": [
                    "stdio-preview",
                    "stdio-passthrough-preview",
                    "stdio-validate",
                    "stdio-validate-passthrough",
                    "stdio-live"
                ],
                "counts_only": true,
                "required_fields": app_server_broker_audit_preview_summary_required_fields()
            }
        },
        "errors": {
            "quota": "json-rpc-error-planned",
            "rate_limit": "json-rpc-error-planned",
            "overload": "json-rpc-error-planned"
        }
    })
}
