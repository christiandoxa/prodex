//! JSON report rendering for app-server broker protocol diagnostics.

use super::*;

#[cfg(test)]
pub(crate) fn app_server_broker_request_summary_json(
    request: &AppServerBrokerRequest,
) -> serde_json::Value {
    let method_kind = app_server_broker_method_kind(Some(&request.raw_method));
    let frame_kind = if request.id.is_some() {
        AppServerBrokerFrameKind::Request
    } else {
        AppServerBrokerFrameKind::Notification
    };
    let lifecycle_stage = app_server_broker_lifecycle_stage(Some(&request.raw_method), frame_kind);
    let lifecycle_schema_file =
        app_server_broker_lifecycle_schema_file(Some(&request.raw_method), frame_kind);
    let mut object = serde_json::Map::new();
    object.insert(
        "jsonrpc".to_string(),
        serde_json::Value::String("2.0".to_string()),
    );
    if let Some(id) = request.id.clone() {
        object.insert("id".to_string(), id);
    }
    object.insert(
        "method".to_string(),
        serde_json::Value::String(request.raw_method.clone()),
    );
    object.insert(
        "params".to_string(),
        serde_json::json!({
            "session_id": request.metadata.session_id,
            "thread_id": request.metadata.thread_id,
            "turn_id": request.metadata.turn_id,
            "item_id": request.metadata.item_id,
        }),
    );
    let diagnostic_value = serde_json::Value::Object(object);
    let affinity_keys = app_server_broker_affinity_keys(&diagnostic_value);
    let primary_affinity = affinity_keys
        .first()
        .map(app_server_broker_affinity_key_json);
    let continuation_affinity =
        app_server_broker_continuation_affinity_summary_json(&diagnostic_value);
    let continuation_decision = app_server_broker_continuation_decision(&diagnostic_value);
    let policy_hint = app_server_broker_policy_hint_json(&diagnostic_value);
    serde_json::json!({
        "id": app_server_broker_redacted_id(request.id.clone()),
        "raw_method": redaction_redact_secret_like_text(&request.raw_method),
        "method": request.method.label(),
        "method_kind": method_kind.label(),
        "lifecycle_stage": lifecycle_stage.map(|stage| stage.label()),
        "lifecycle_schema_file": lifecycle_schema_file,
        "continuation_affinity": continuation_affinity,
        "continuation_decision": continuation_decision.label(),
        "policy_hint": policy_hint,
        "primary_affinity": primary_affinity,
        "affinity_keys": affinity_keys.iter().map(app_server_broker_affinity_key_json).collect::<Vec<_>>(),
        "metadata": app_server_broker_metadata_json(&request.metadata)
    })
}

#[cfg(test)]
pub(crate) fn app_server_broker_response_summary_json(value: &Value) -> serde_json::Value {
    let summary = app_server_broker_diagnostic_summary(value);
    let object = value.as_object();
    let has_error = object.is_some_and(|object| object.contains_key("error"));
    let error = object
        .and_then(|object| object.get("error"))
        .and_then(Value::as_object);
    let affinity_keys = app_server_broker_affinity_keys(value);
    let primary_affinity = affinity_keys
        .first()
        .map(app_server_broker_affinity_key_json);
    let continuation_affinity = app_server_broker_continuation_affinity_summary_json(value);
    let continuation_decision = app_server_broker_continuation_decision(value);
    let policy_hint = app_server_broker_policy_hint_json(value);
    serde_json::json!({
        "valid_jsonrpc": summary.valid_jsonrpc,
        "frame_kind": summary.frame_kind.label(),
        "id": app_server_broker_redacted_id(object.and_then(|object| object.get("id")).cloned()),
        "has_result": object.is_some_and(|object| object.contains_key("result")),
        "has_error": has_error,
        "error_code": error.and_then(|error| error.get("code")).cloned(),
        "error_message": app_server_broker_redacted_string_value(error.and_then(|error| error.get("message")).cloned()),
        "continuation_affinity": continuation_affinity,
        "continuation_decision": continuation_decision.label(),
        "policy_hint": policy_hint,
        "primary_affinity": primary_affinity,
        "affinity_keys": affinity_keys.iter().map(app_server_broker_affinity_key_json).collect::<Vec<_>>(),
        "metadata": app_server_broker_metadata_json(&summary.metadata)
    })
}

pub(crate) fn app_server_broker_diagnostic_summary_json(value: &Value) -> serde_json::Value {
    let summary = app_server_broker_diagnostic_summary(value);
    let method_kind = app_server_broker_method_kind(summary.method.as_deref());
    let lifecycle_stage =
        app_server_broker_lifecycle_stage(summary.method.as_deref(), summary.frame_kind);
    let lifecycle_schema_file =
        app_server_broker_lifecycle_schema_file(summary.method.as_deref(), summary.frame_kind);
    let affinity_keys = app_server_broker_affinity_keys(value);
    let primary_affinity = affinity_keys.first().map(|key| {
        serde_json::json!({
            "kind": key.kind.label(),
            "value": key.value,
        })
    });
    let continuation_affinity = app_server_broker_continuation_affinity_summary_json(value);
    let continuation_decision = app_server_broker_continuation_decision(value);
    let policy_hint = app_server_broker_policy_hint_json(value);
    serde_json::json!({
        "valid_jsonrpc": summary.valid_jsonrpc,
        "frame_kind": summary.frame_kind.label(),
        "id": app_server_broker_redacted_id(summary.id),
        "method": app_server_broker_redacted_string(summary.method),
        "method_kind": method_kind.label(),
        "is_lifecycle_method": matches!(method_kind, AppServerBrokerMethodKind::Lifecycle),
        "lifecycle_stage": lifecycle_stage.map(|stage| stage.label()),
        "lifecycle_schema_file": lifecycle_schema_file,
        "continuation_affinity": continuation_affinity,
        "continuation_decision": continuation_decision.label(),
        "policy_hint": policy_hint,
        "primary_affinity": primary_affinity,
        "affinity_keys": affinity_keys.iter().map(app_server_broker_affinity_key_json).collect::<Vec<_>>(),
        "invalid_reason": summary.invalid_reason,
        "metadata": app_server_broker_metadata_json(&summary.metadata)
    })
}

fn app_server_broker_redacted_id(id: Option<Value>) -> Option<Value> {
    app_server_broker_redacted_string_value(id)
}

fn app_server_broker_redacted_string(value: Option<String>) -> Option<String> {
    value.map(|value| redaction_redact_secret_like_text(&value))
}

fn app_server_broker_redacted_string_value(value: Option<Value>) -> Option<Value> {
    value.map(|value| match value {
        Value::String(value) => Value::String(redaction_redact_secret_like_text(&value)),
        value => value,
    })
}

fn app_server_broker_metadata_json(metadata: &AppServerBrokerMetadata) -> serde_json::Value {
    serde_json::json!({
        "session_id": app_server_broker_redacted_string(metadata.session_id.clone()),
        "thread_id": app_server_broker_redacted_string(metadata.thread_id.clone()),
        "turn_id": app_server_broker_redacted_string(metadata.turn_id.clone()),
        "item_id": app_server_broker_redacted_string(metadata.item_id.clone()),
    })
}

pub(crate) fn app_server_broker_affinity_key_json(
    key: &AppServerBrokerAffinityKey,
) -> serde_json::Value {
    serde_json::json!({
        "kind": key.kind.label(),
        "value": redaction_redact_secret_like_text(&key.value),
    })
}
