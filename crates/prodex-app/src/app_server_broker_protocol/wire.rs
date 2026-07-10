//! JSON-RPC wire-frame classification and validation for app-server broker diagnostics.

use super::*;

pub(crate) fn app_server_broker_frame_kind(value: &Value) -> AppServerBrokerFrameKind {
    let Some(object) = value.as_object() else {
        return AppServerBrokerFrameKind::Invalid;
    };
    if !app_server_broker_has_valid_wire_jsonrpc(object) {
        return AppServerBrokerFrameKind::Invalid;
    }
    if !app_server_broker_has_valid_wire_id(object) {
        return AppServerBrokerFrameKind::Invalid;
    }
    if !app_server_broker_has_valid_wire_params(object) {
        return AppServerBrokerFrameKind::Invalid;
    }
    if !app_server_broker_has_valid_wire_error(object) {
        return AppServerBrokerFrameKind::Invalid;
    }
    if !app_server_broker_has_valid_wire_error_code(object) {
        return AppServerBrokerFrameKind::Invalid;
    }
    if !app_server_broker_has_valid_wire_error_message(object) {
        return AppServerBrokerFrameKind::Invalid;
    }
    if object.contains_key("method") && object.get("method").and_then(Value::as_str).is_none() {
        return AppServerBrokerFrameKind::Invalid;
    }
    if !app_server_broker_has_valid_wire_method_name(object) {
        return AppServerBrokerFrameKind::Invalid;
    }
    let has_method = object.get("method").and_then(Value::as_str).is_some();
    let has_id = object.contains_key("id");
    let has_result = object.contains_key("result");
    let has_error = object.contains_key("error");
    if has_result && has_error {
        AppServerBrokerFrameKind::Invalid
    } else if has_method && (has_result || has_error) {
        AppServerBrokerFrameKind::Invalid
    } else if !has_method && (has_result || has_error) && !has_id {
        AppServerBrokerFrameKind::Invalid
    } else if has_method && has_id {
        AppServerBrokerFrameKind::Request
    } else if has_method {
        AppServerBrokerFrameKind::Notification
    } else if has_result || has_error {
        AppServerBrokerFrameKind::Response
    } else {
        AppServerBrokerFrameKind::Invalid
    }
}

pub(crate) fn app_server_broker_invalid_reason(value: &Value) -> Option<&'static str> {
    if value.is_array() {
        return Some("batch_frame_unsupported");
    }
    let Some(object) = value.as_object() else {
        return Some("non_object_frame");
    };
    if object
        .get("jsonrpc")
        .is_some_and(|jsonrpc| jsonrpc.as_str() != Some("2.0"))
    {
        return Some("non_jsonrpc_version");
    }
    if !app_server_broker_has_valid_wire_id(object) {
        return Some("non_scalar_id");
    }
    if !app_server_broker_has_valid_wire_params(object) {
        return Some("non_container_params");
    }
    if !app_server_broker_has_valid_wire_error(object) {
        return Some("non_object_error");
    }
    if !app_server_broker_has_valid_wire_error_code(object) {
        return Some("non_integer_error_code");
    }
    if !app_server_broker_has_valid_wire_error_message(object) {
        return Some("non_string_error_message");
    }
    if object.contains_key("method") && object.get("method").and_then(Value::as_str).is_none() {
        return Some("non_string_method");
    }
    if !app_server_broker_has_valid_wire_method_name(object) {
        return Some("invalid_method_name");
    }
    let has_method = object.get("method").and_then(Value::as_str).is_some();
    let has_result = object.contains_key("result");
    let has_error = object.contains_key("error");
    if has_result && has_error {
        Some("result_with_error")
    } else if has_method && (has_result || has_error) {
        Some("method_with_result_or_error")
    } else if !has_method && (has_result || has_error) && !object.contains_key("id") {
        Some("missing_response_id")
    } else if !has_method && !(has_result || has_error) {
        Some("missing_method_and_response_payload")
    } else {
        None
    }
}

pub(super) fn app_server_broker_has_valid_wire_jsonrpc(
    object: &serde_json::Map<String, Value>,
) -> bool {
    object
        .get("jsonrpc")
        .map(|jsonrpc| jsonrpc.as_str() == Some("2.0"))
        .unwrap_or(true)
}

fn app_server_broker_has_valid_wire_id(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("id")
        .map(|id| id.is_string() || id.is_number() || id.is_null())
        .unwrap_or(true)
}

fn app_server_broker_has_valid_wire_params(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("params")
        .map(|params| params.is_object() || params.is_array())
        .unwrap_or(true)
}

fn app_server_broker_has_valid_wire_error(object: &serde_json::Map<String, Value>) -> bool {
    object.get("error").map(Value::is_object).unwrap_or(true)
}

fn app_server_broker_has_valid_wire_error_code(object: &serde_json::Map<String, Value>) -> bool {
    let Some(error) = object.get("error").and_then(Value::as_object) else {
        return true;
    };
    error
        .get("code")
        .is_some_and(|code| code.as_i64().is_some() || code.as_u64().is_some())
}

fn app_server_broker_has_valid_wire_error_message(object: &serde_json::Map<String, Value>) -> bool {
    let Some(error) = object.get("error").and_then(Value::as_object) else {
        return true;
    };
    error.get("message").is_some_and(Value::is_string)
}

fn app_server_broker_has_valid_wire_method_name(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("method")
        .and_then(Value::as_str)
        .map(|method| {
            let method = method.trim();
            !method.is_empty() && !method.starts_with("rpc.")
        })
        .unwrap_or(true)
}
