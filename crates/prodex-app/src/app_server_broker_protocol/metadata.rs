//! Metadata extraction for app-server broker protocol frames.

use super::wire::app_server_broker_has_valid_wire_jsonrpc;
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AppServerBrokerMetadata {
    pub session_id: Option<String>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
}

impl AppServerBrokerMetadata {
    pub(crate) fn is_empty(&self) -> bool {
        self.session_id.is_none()
            && self.thread_id.is_none()
            && self.turn_id.is_none()
            && self.item_id.is_none()
    }
}

pub(crate) fn app_server_broker_metadata_from_value(value: &Value) -> AppServerBrokerMetadata {
    let object = match value.as_object() {
        Some(object) if app_server_broker_has_valid_wire_jsonrpc(object) => object,
        _ => return AppServerBrokerMetadata::default(),
    };
    let params = object.get("params");
    let result = object.get("result");
    let session_id = app_server_broker_first_string(
        &[params, result],
        &[
            &["session_id"],
            &["sessionId"],
            &["client_metadata", "session_id"],
            &["client_metadata", "sessionId"],
            &["context", "session_id"],
            &["context", "sessionId"],
            &["thread", "session_id"],
            &["thread", "sessionId"],
        ],
    );
    let thread_id = app_server_broker_first_string(
        &[params, result],
        &[
            &["thread_id"],
            &["threadId"],
            &["client_metadata", "thread_id"],
            &["client_metadata", "threadId"],
            &["thread", "id"],
            &["context", "thread_id"],
            &["context", "threadId"],
        ],
    );
    let turn_id = app_server_broker_first_string(
        &[params, result],
        &[
            &["turn_id"],
            &["turnId"],
            &["client_metadata", "turn_id"],
            &["client_metadata", "turnId"],
            &["turn", "id"],
            &["context", "turn_id"],
            &["context", "turnId"],
        ],
    );
    let item_id = app_server_broker_first_string(
        &[params, result],
        &[
            &["item_id"],
            &["itemId"],
            &["client_metadata", "item_id"],
            &["client_metadata", "itemId"],
            &["item", "id"],
            &["context", "item_id"],
            &["context", "itemId"],
        ],
    );
    AppServerBrokerMetadata {
        session_id,
        thread_id,
        turn_id,
        item_id,
    }
}

fn app_server_broker_first_string(roots: &[Option<&Value>], paths: &[&[&str]]) -> Option<String> {
    for root in roots {
        let Some(root) = root else {
            continue;
        };
        for path in paths {
            let mut current = *root;
            let mut valid = true;
            for segment in *path {
                match current.get(*segment) {
                    Some(next) => current = next,
                    None => {
                        valid = false;
                        break;
                    }
                }
            }
            if valid
                && let Some(value) = current.as_str().map(str::trim)
                && !value.is_empty()
            {
                return Some(value.to_string());
            }
        }
    }
    None
}
