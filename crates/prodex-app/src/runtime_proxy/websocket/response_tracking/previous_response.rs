use super::*;

pub(super) struct RuntimeWebsocketCommittedPreviousResponseNotFoundRequest<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: &'a str,
    pub(super) request_previous_response_id: Option<&'a str>,
    pub(super) committed_response_ids: &'a BTreeSet<String>,
}

pub(super) fn record_runtime_websocket_committed_previous_response_not_found(
    request: RuntimeWebsocketCommittedPreviousResponseNotFoundRequest<'_>,
) {
    let RuntimeWebsocketCommittedPreviousResponseNotFoundRequest {
        request_id,
        shared,
        profile_name,
        request_previous_response_id,
        committed_response_ids,
    } = request;

    let mut dead_response_ids = committed_response_ids.iter().cloned().collect::<Vec<_>>();
    if let Some(previous_response_id) = request_previous_response_id {
        dead_response_ids.push(previous_response_id.to_string());
    }
    let _ = clear_runtime_dead_response_bindings(
        shared,
        profile_name,
        &dead_response_ids,
        "previous_response_not_found_after_commit",
    );
    runtime_proxy_log_previous_response_stale_continuation(
        shared,
        RuntimePreviousResponseLogContext {
            request_id,
            transport: "websocket",
            route: "websocket",
            websocket_session: None,
            via: None,
        },
        profile_name,
    );
    runtime_proxy_log_chain_dead_upstream_confirmed(
        shared,
        RuntimeProxyChainLog {
            request_id,
            transport: "websocket",
            route: "websocket",
            websocket_session: None,
            profile_name,
            previous_response_id: request_previous_response_id,
            reason: "previous_response_not_found_locked_affinity",
            via: None,
        },
        Some("post_commit"),
    );
}
