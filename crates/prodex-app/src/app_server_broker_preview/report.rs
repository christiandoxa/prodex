//! Preview report aggregation for app-server broker diagnostics.

use serde_json::Value;

pub(super) fn app_server_broker_preview_report_from_previews(previews: Vec<Value>) -> Value {
    let parsed = previews
        .iter()
        .filter(|entry| entry["preview"]["parse_ok"] == serde_json::Value::Bool(true))
        .count();
    let failed = previews.len().saturating_sub(parsed);
    let mut request_count = 0usize;
    let mut notification_count = 0usize;
    let mut response_count = 0usize;
    let mut invalid_count = 0usize;
    let mut lifecycle_count = 0usize;
    let mut other_count = 0usize;
    let mut absent_count = 0usize;
    let mut fresh_count = 0usize;
    let mut continue_session_count = 0usize;
    let mut continue_thread_count = 0usize;
    let mut continue_turn_count = 0usize;
    let mut fresh_selection_ok_count = 0usize;
    let mut preserve_session_affinity_count = 0usize;
    let mut preserve_thread_affinity_count = 0usize;
    let mut preserve_turn_affinity_count = 0usize;
    let mut precommit_count = 0usize;
    let mut turn_committed_count = 0usize;
    let mut rotation_open_count = 0usize;
    let mut rotation_closed_count = 0usize;
    let mut fresh_select_ok_count = 0usize;
    let mut preserve_session_owner_count = 0usize;
    let mut preserve_thread_owner_count = 0usize;
    let mut preserve_turn_owner_count = 0usize;
    let mut affinity_required_count = 0usize;
    let mut rotation_allowed_count = 0usize;
    let mut preserves_owner_count = 0usize;
    let mut owner_none_count = 0usize;
    let mut owner_session_count = 0usize;
    let mut owner_thread_count = 0usize;
    let mut owner_turn_count = 0usize;
    let mut non_jsonrpc_version_count = 0usize;
    let mut batch_frame_unsupported_count = 0usize;
    let mut non_object_frame_count = 0usize;
    let mut non_scalar_id_count = 0usize;
    let mut non_container_params_count = 0usize;
    let mut non_object_error_count = 0usize;
    let mut non_integer_error_code_count = 0usize;
    let mut non_string_error_message_count = 0usize;
    let mut non_string_method_count = 0usize;
    let mut invalid_method_name_count = 0usize;
    let mut result_with_error_count = 0usize;
    let mut missing_response_id_count = 0usize;
    let mut method_with_result_or_error_count = 0usize;
    let mut missing_method_and_response_payload_count = 0usize;
    for entry in &previews {
        match entry["preview"]["summary"]["frame_kind"].as_str() {
            Some("request") => request_count += 1,
            Some("notification") => notification_count += 1,
            Some("response") => response_count += 1,
            Some("invalid") => invalid_count += 1,
            _ => {}
        }
        match entry["preview"]["summary"]["method_kind"].as_str() {
            Some("lifecycle") => lifecycle_count += 1,
            Some("other") => other_count += 1,
            Some("absent") => absent_count += 1,
            _ => {}
        }
        match entry["preview"]["summary"]["continuation_decision"].as_str() {
            Some("fresh") => fresh_count += 1,
            Some("continue-session") => continue_session_count += 1,
            Some("continue-thread") => continue_thread_count += 1,
            Some("continue-turn") => continue_turn_count += 1,
            _ => {}
        }
        match entry["preview"]["summary"]["policy_hint"]["mode"].as_str() {
            Some("fresh-selection-ok") => fresh_selection_ok_count += 1,
            Some("preserve-session-affinity") => preserve_session_affinity_count += 1,
            Some("preserve-thread-affinity") => preserve_thread_affinity_count += 1,
            Some("preserve-turn-affinity") => preserve_turn_affinity_count += 1,
            _ => {}
        }
        match entry["preview"]["summary"]["policy_hint"]["commit_boundary"].as_str() {
            Some("precommit") => precommit_count += 1,
            Some("turn-committed") => turn_committed_count += 1,
            _ => {}
        }
        match entry["preview"]["summary"]["policy_hint"]["rotation_window"].as_str() {
            Some("open") => rotation_open_count += 1,
            Some("closed") => rotation_closed_count += 1,
            _ => {}
        }
        match entry["preview"]["summary"]["policy_hint"]["routing_hint"].as_str() {
            Some("fresh-select-ok") => fresh_select_ok_count += 1,
            Some("preserve-session-owner") => preserve_session_owner_count += 1,
            Some("preserve-thread-owner") => preserve_thread_owner_count += 1,
            Some("preserve-turn-owner") => preserve_turn_owner_count += 1,
            _ => {}
        }
        if entry["preview"]["summary"]["policy_hint"]["affinity_required"]
            == serde_json::Value::Bool(true)
        {
            affinity_required_count += 1;
        }
        if entry["preview"]["summary"]["policy_hint"]["rotation_allowed"]
            == serde_json::Value::Bool(true)
        {
            rotation_allowed_count += 1;
        }
        if entry["preview"]["summary"]["policy_hint"]["preserves_owner"]
            == serde_json::Value::Bool(true)
        {
            preserves_owner_count += 1;
        }
        match entry["preview"]["summary"]["continuation_affinity"]["owner_kind"].as_str() {
            Some("none") => owner_none_count += 1,
            Some("session") => owner_session_count += 1,
            Some("thread") => owner_thread_count += 1,
            Some("turn") => owner_turn_count += 1,
            _ => {}
        }
        match entry["preview"]["summary"]["invalid_reason"].as_str() {
            Some("non_jsonrpc_version") => non_jsonrpc_version_count += 1,
            Some("batch_frame_unsupported") => batch_frame_unsupported_count += 1,
            Some("non_object_frame") => non_object_frame_count += 1,
            Some("non_scalar_id") => non_scalar_id_count += 1,
            Some("non_container_params") => non_container_params_count += 1,
            Some("non_object_error") => non_object_error_count += 1,
            Some("non_integer_error_code") => non_integer_error_code_count += 1,
            Some("non_string_error_message") => non_string_error_message_count += 1,
            Some("non_string_method") => non_string_method_count += 1,
            Some("invalid_method_name") => invalid_method_name_count += 1,
            Some("result_with_error") => result_with_error_count += 1,
            Some("missing_response_id") => missing_response_id_count += 1,
            Some("method_with_result_or_error") => method_with_result_or_error_count += 1,
            Some("missing_method_and_response_payload") => {
                missing_method_and_response_payload_count += 1
            }
            _ => {}
        }
    }
    serde_json::json!({
        "line_count": previews.len(),
        "parsed_count": parsed,
        "error_count": failed,
        "frame_kind_counts": {
            "request": request_count,
            "notification": notification_count,
            "response": response_count,
            "invalid": invalid_count,
        },
        "method_kind_counts": {
            "lifecycle": lifecycle_count,
            "other": other_count,
            "absent": absent_count,
        },
        "continuation_decision_counts": {
            "fresh": fresh_count,
            "continue-session": continue_session_count,
            "continue-thread": continue_thread_count,
            "continue-turn": continue_turn_count,
        },
        "policy_mode_counts": {
            "fresh-selection-ok": fresh_selection_ok_count,
            "preserve-session-affinity": preserve_session_affinity_count,
            "preserve-thread-affinity": preserve_thread_affinity_count,
            "preserve-turn-affinity": preserve_turn_affinity_count,
        },
        "commit_boundary_counts": {
            "precommit": precommit_count,
            "turn-committed": turn_committed_count,
        },
        "rotation_window_counts": {
            "open": rotation_open_count,
            "closed": rotation_closed_count,
        },
        "routing_hint_counts": {
            "fresh-select-ok": fresh_select_ok_count,
            "preserve-session-owner": preserve_session_owner_count,
            "preserve-thread-owner": preserve_thread_owner_count,
            "preserve-turn-owner": preserve_turn_owner_count,
        },
        "policy_flag_counts": {
            "affinity_required": affinity_required_count,
            "rotation_allowed": rotation_allowed_count,
            "preserves_owner": preserves_owner_count,
        },
        "owner_kind_counts": {
            "none": owner_none_count,
            "session": owner_session_count,
            "thread": owner_thread_count,
            "turn": owner_turn_count,
        },
        "invalid_reason_counts": {
            "non_jsonrpc_version": non_jsonrpc_version_count,
            "batch_frame_unsupported": batch_frame_unsupported_count,
            "non_object_frame": non_object_frame_count,
            "non_scalar_id": non_scalar_id_count,
            "non_container_params": non_container_params_count,
            "non_object_error": non_object_error_count,
            "non_integer_error_code": non_integer_error_code_count,
            "non_string_error_message": non_string_error_message_count,
            "non_string_method": non_string_method_count,
            "invalid_method_name": invalid_method_name_count,
            "result_with_error": result_with_error_count,
            "missing_response_id": missing_response_id_count,
            "method_with_result_or_error": method_with_result_or_error_count,
            "missing_method_and_response_payload": missing_method_and_response_payload_count,
        },
        "previews": previews,
    })
}
