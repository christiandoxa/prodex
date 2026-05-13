use crate::runtime_proxy::{
    build_runtime_proxy_json_error_parts, runtime_proxy_local_selection_failure_message,
};
use crate::runtime_proxy_shared::RuntimeResponsesReply;

pub(super) enum RuntimeResponsesLocalSelectionAction {
    ReturnServiceUnavailable,
    Rotate,
}

pub(super) fn runtime_responses_local_selection_action(
    releasable: bool,
) -> RuntimeResponsesLocalSelectionAction {
    if releasable {
        RuntimeResponsesLocalSelectionAction::Rotate
    } else {
        RuntimeResponsesLocalSelectionAction::ReturnServiceUnavailable
    }
}

pub(super) fn runtime_responses_local_selection_failure_reply() -> RuntimeResponsesReply {
    RuntimeResponsesReply::Buffered(build_runtime_proxy_json_error_parts(
        503,
        "service_unavailable",
        runtime_proxy_local_selection_failure_message(),
    ))
}
