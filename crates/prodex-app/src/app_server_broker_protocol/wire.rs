//! JSON-RPC wire-frame classification and validation for app-server broker diagnostics.

use super::AppServerBrokerFrameKind;
use serde_json::Value;

pub(crate) const APP_SERVER_BROKER_MAX_BATCH_ITEMS: usize = 4_096;

#[cfg(feature = "mojo-core")]
pub(crate) fn app_server_broker_frame_kind(value: &Value) -> AppServerBrokerFrameKind {
    if value.is_array() {
        return if app_server_broker_invalid_reason(value).is_none() {
            AppServerBrokerFrameKind::Batch
        } else {
            AppServerBrokerFrameKind::Invalid
        };
    }
    let Some(object) = value.as_object() else {
        return AppServerBrokerFrameKind::Invalid;
    };
    super::mojo::frame_kind(super::mojo::wire_plan(object).frame_kind)
}

#[cfg(not(feature = "mojo-core"))]
pub(crate) fn app_server_broker_frame_kind(value: &Value) -> AppServerBrokerFrameKind {
    if value.is_array() {
        return if app_server_broker_invalid_reason(value).is_none() {
            AppServerBrokerFrameKind::Batch
        } else {
            AppServerBrokerFrameKind::Invalid
        };
    }
    let Some(object) = value.as_object() else {
        return AppServerBrokerFrameKind::Invalid;
    };
    if !app_server_broker_has_valid_wire_object(object) {
        return AppServerBrokerFrameKind::Invalid;
    }
    app_server_broker_object_frame_kind(object)
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_has_valid_wire_object(object: &serde_json::Map<String, Value>) -> bool {
    app_server_broker_has_valid_wire_jsonrpc(object)
        && app_server_broker_has_valid_wire_id(object)
        && app_server_broker_has_valid_wire_params(object)
        && app_server_broker_has_valid_wire_error(object)
        && app_server_broker_has_valid_wire_error_code(object)
        && app_server_broker_has_valid_wire_error_message(object)
        && (!object.contains_key("method")
            || object.get("method").and_then(Value::as_str).is_some())
        && app_server_broker_has_valid_wire_method_name(object)
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_object_frame_kind(
    object: &serde_json::Map<String, Value>,
) -> AppServerBrokerFrameKind {
    let has_method = object.get("method").and_then(Value::as_str).is_some();
    let has_id = object.contains_key("id");
    let has_result = object.contains_key("result");
    let has_error = object.contains_key("error");
    let has_response_payload = has_result || has_error;
    if (has_result && has_error)
        || (has_method && has_response_payload)
        || (!has_method && has_response_payload && !has_id)
    {
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
    #[cfg(feature = "mojo-core")]
    {
        if let Some(batch) = value.as_array() {
            return app_server_broker_invalid_batch_reason(batch);
        }
        let Some(object) = value.as_object() else {
            return Some("non_object_frame");
        };
        super::mojo::invalid_reason(super::mojo::wire_plan(object).invalid_reason)
    }
    #[cfg(not(feature = "mojo-core"))]
    {
        if let Some(batch) = value.as_array() {
            return app_server_broker_invalid_batch_reason(batch);
        }
        let Some(object) = value.as_object() else {
            return Some("non_object_frame");
        };
        app_server_broker_invalid_object_reason(object)
    }
}

fn app_server_broker_invalid_batch_reason(batch: &[Value]) -> Option<&'static str> {
    if batch.is_empty() {
        return Some("empty_batch");
    }
    if batch.len() > APP_SERVER_BROKER_MAX_BATCH_ITEMS {
        return Some("batch_too_large");
    }
    if batch.iter().any(Value::is_array) {
        return Some("nested_batch");
    }
    if batch
        .iter()
        .any(|frame| app_server_broker_invalid_reason(frame).is_some())
    {
        return Some("invalid_batch_member");
    }
    None
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_invalid_object_reason(
    object: &serde_json::Map<String, Value>,
) -> Option<&'static str> {
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
    app_server_broker_invalid_payload_reason(object)
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_invalid_payload_reason(
    object: &serde_json::Map<String, Value>,
) -> Option<&'static str> {
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

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_has_valid_wire_id(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("id")
        .map(|id| id.is_string() || id.is_number() || id.is_null())
        .unwrap_or(true)
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_has_valid_wire_params(object: &serde_json::Map<String, Value>) -> bool {
    object
        .get("params")
        .map(|params| params.is_object() || params.is_array())
        .unwrap_or(true)
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_has_valid_wire_error(object: &serde_json::Map<String, Value>) -> bool {
    object.get("error").map(Value::is_object).unwrap_or(true)
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_has_valid_wire_error_code(object: &serde_json::Map<String, Value>) -> bool {
    let Some(error) = object.get("error").and_then(Value::as_object) else {
        return true;
    };
    error
        .get("code")
        .is_some_and(|code| code.as_i64().is_some() || code.as_u64().is_some())
}

#[cfg(not(feature = "mojo-core"))]
fn app_server_broker_has_valid_wire_error_message(object: &serde_json::Map<String, Value>) -> bool {
    let Some(error) = object.get("error").and_then(Value::as_object) else {
        return true;
    };
    error.get("message").is_some_and(Value::is_string)
}

#[cfg(not(feature = "mojo-core"))]
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
