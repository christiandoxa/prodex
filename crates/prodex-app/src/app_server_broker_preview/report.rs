//! Preview report aggregation for app-server broker diagnostics.

use serde_json::Value;
use std::collections::BTreeMap;

type PreviewCounts = BTreeMap<String, usize>;

pub(super) fn app_server_broker_preview_report_from_previews(previews: Vec<Value>) -> Value {
    let is_first_wire_frame =
        |entry: &&Value| entry["batch_index"].as_u64().is_none_or(|index| index == 0);
    let line_count = previews.iter().filter(is_first_wire_frame).count();
    let parsed = previews
        .iter()
        .filter(is_first_wire_frame)
        .filter(|entry| entry["preview"]["parse_ok"] == serde_json::Value::Bool(true))
        .count();
    let counts = preview_counts(&previews);

    serde_json::json!({
        "line_count": line_count,
        "parsed_count": parsed,
        "error_count": line_count.saturating_sub(parsed),
        "frame_kind_counts": {
            "batch": count(&counts, "batch"),
            "request": count(&counts, "request"),
            "notification": count(&counts, "notification"),
            "response": count(&counts, "response"),
            "invalid": count(&counts, "invalid"),
        },
        "method_kind_counts": {
            "lifecycle": count(&counts, "lifecycle"),
            "other": count(&counts, "other"),
            "absent": count(&counts, "absent"),
        },
        "continuation_decision_counts": {
            "fresh": count(&counts, "fresh"),
            "continue-session": count(&counts, "continue-session"),
            "continue-thread": count(&counts, "continue-thread"),
            "continue-turn": count(&counts, "continue-turn"),
        },
        "policy_mode_counts": {
            "fresh-selection-ok": count(&counts, "fresh-selection-ok"),
            "preserve-session-affinity": count(&counts, "preserve-session-affinity"),
            "preserve-thread-affinity": count(&counts, "preserve-thread-affinity"),
            "preserve-turn-affinity": count(&counts, "preserve-turn-affinity"),
        },
        "commit_boundary_counts": {
            "precommit": count(&counts, "precommit"),
            "turn-committed": count(&counts, "turn-committed"),
        },
        "rotation_window_counts": {
            "open": count(&counts, "open"),
            "closed": count(&counts, "closed"),
        },
        "routing_hint_counts": {
            "fresh-select-ok": count(&counts, "fresh-select-ok"),
            "preserve-session-owner": count(&counts, "preserve-session-owner"),
            "preserve-thread-owner": count(&counts, "preserve-thread-owner"),
            "preserve-turn-owner": count(&counts, "preserve-turn-owner"),
        },
        "policy_flag_counts": {
            "affinity_required": count(&counts, "affinity_required"),
            "rotation_allowed": count(&counts, "rotation_allowed"),
            "preserves_owner": count(&counts, "preserves_owner"),
        },
        "owner_kind_counts": {
            "none": count(&counts, "none"),
            "session": count(&counts, "session"),
            "thread": count(&counts, "thread"),
            "turn": count(&counts, "turn"),
        },
        "invalid_reason_counts": {
            "non_jsonrpc_version": count(&counts, "non_jsonrpc_version"),
            "empty_batch": count(&counts, "empty_batch"),
            "batch_too_large": count(&counts, "batch_too_large"),
            "nested_batch": count(&counts, "nested_batch"),
            "invalid_batch_member": count(&counts, "invalid_batch_member"),
            "non_object_frame": count(&counts, "non_object_frame"),
            "non_scalar_id": count(&counts, "non_scalar_id"),
            "non_container_params": count(&counts, "non_container_params"),
            "non_object_error": count(&counts, "non_object_error"),
            "non_integer_error_code": count(&counts, "non_integer_error_code"),
            "non_string_error_message": count(&counts, "non_string_error_message"),
            "non_string_method": count(&counts, "non_string_method"),
            "invalid_method_name": count(&counts, "invalid_method_name"),
            "result_with_error": count(&counts, "result_with_error"),
            "missing_response_id": count(&counts, "missing_response_id"),
            "method_with_result_or_error": count(&counts, "method_with_result_or_error"),
            "missing_method_and_response_payload": count(
                &counts,
                "missing_method_and_response_payload",
            ),
        },
        "previews": previews,
    })
}

fn preview_counts(previews: &[Value]) -> PreviewCounts {
    let mut counts = PreviewCounts::new();
    for entry in previews {
        count_frame_summary(&mut counts, entry);
        count_policy_summary(&mut counts, entry);
        count_invalid_reason(&mut counts, entry);
    }
    counts
}

fn count_frame_summary(counts: &mut PreviewCounts, entry: &Value) {
    if entry["batch_index"].as_u64() == Some(0) {
        increment(counts, "batch");
    }
    if let Some(kind @ ("batch" | "request" | "notification" | "response" | "invalid")) =
        entry["preview"]["summary"]["frame_kind"].as_str()
    {
        increment(counts, kind);
    }
    if let Some(kind @ ("lifecycle" | "other" | "absent")) =
        entry["preview"]["summary"]["method_kind"].as_str()
    {
        increment(counts, kind);
    }
    if let Some(kind @ ("fresh" | "continue-session" | "continue-thread" | "continue-turn")) =
        entry["preview"]["summary"]["continuation_decision"].as_str()
    {
        increment(counts, kind);
    }
}

fn count_policy_summary(counts: &mut PreviewCounts, entry: &Value) {
    let policy = &entry["preview"]["summary"]["policy_hint"];
    if let Some(
        kind @ ("fresh-selection-ok"
        | "preserve-session-affinity"
        | "preserve-thread-affinity"
        | "preserve-turn-affinity"),
    ) = policy["mode"].as_str()
    {
        increment(counts, kind);
    }
    if let Some(kind @ ("precommit" | "turn-committed")) = policy["commit_boundary"].as_str() {
        increment(counts, kind);
    }
    if let Some(kind @ ("open" | "closed")) = policy["rotation_window"].as_str() {
        increment(counts, kind);
    }
    if let Some(
        kind @ ("fresh-select-ok"
        | "preserve-session-owner"
        | "preserve-thread-owner"
        | "preserve-turn-owner"),
    ) = policy["routing_hint"].as_str()
    {
        increment(counts, kind);
    }
    for (field, key) in [
        ("affinity_required", "affinity_required"),
        ("rotation_allowed", "rotation_allowed"),
        ("preserves_owner", "preserves_owner"),
    ] {
        if policy[field] == Value::Bool(true) {
            increment(counts, key);
        }
    }
    if let Some(kind @ ("none" | "session" | "thread" | "turn")) =
        entry["preview"]["summary"]["continuation_affinity"]["owner_kind"].as_str()
    {
        increment(counts, kind);
    }
}

fn count_invalid_reason(counts: &mut PreviewCounts, entry: &Value) {
    if let Some(
        kind @ ("non_jsonrpc_version"
        | "empty_batch"
        | "batch_too_large"
        | "nested_batch"
        | "invalid_batch_member"
        | "non_object_frame"
        | "non_scalar_id"
        | "non_container_params"
        | "non_object_error"),
    ) = entry["preview"]["summary"]["invalid_reason"].as_str()
    {
        increment(counts, kind);
    } else {
        count_invalid_payload_reason(counts, entry);
    }
}

fn count_invalid_payload_reason(counts: &mut PreviewCounts, entry: &Value) {
    if let Some(
        kind @ ("non_integer_error_code"
        | "non_string_error_message"
        | "non_string_method"
        | "invalid_method_name"
        | "result_with_error"
        | "missing_response_id"
        | "method_with_result_or_error"
        | "missing_method_and_response_payload"),
    ) = entry["preview"]["summary"]["invalid_reason"].as_str()
    {
        increment(counts, kind);
    }
}

fn increment(counts: &mut PreviewCounts, key: &str) {
    *counts.entry(key.to_owned()).or_default() += 1;
}

fn count(counts: &PreviewCounts, key: &str) -> usize {
    counts.get(key).copied().unwrap_or_default()
}
