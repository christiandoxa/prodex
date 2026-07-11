//! Standard-route success forwarding helpers.

use super::*;

pub(super) fn forward_runtime_compact_success_attempt(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_session_id: Option<&str>,
    response: reqwest::Response,
    compact_request: bool,
) -> Result<RuntimeStandardAttempt> {
    let response_turn_state = compact_request
        .then(|| runtime_proxy_header_value(response.headers(), "x-codex-turn-state"))
        .flatten();
    let response_result = if compact_request {
        await_runtime_proxy_async_task(
            shared,
            "compact_forward_response",
            forward_runtime_proxy_response_with_limit(
                response,
                Vec::new(),
                RUNTIME_PROXY_COMPACT_BUFFERED_RESPONSE_MAX_BYTES,
            ),
        )
    } else {
        await_runtime_proxy_async_task(
            shared,
            "compact_forward_response",
            forward_runtime_proxy_response(response, Vec::new()),
        )
    };
    let response = match response_result {
        Ok(response) => response,
        Err(err) => {
            note_runtime_profile_transport_failure(
                shared,
                profile_name,
                RuntimeRouteKind::Compact,
                "compact_forward_response",
                &err,
            );
            if is_runtime_proxy_transport_failure(&err) {
                return Ok(RuntimeStandardAttempt::TransportFailed {
                    profile_name: profile_name.to_string(),
                    stage: "compact_forward_response",
                });
            }
            return Err(err);
        }
    };
    remember_runtime_session_id(
        shared,
        profile_name,
        request_session_id,
        if compact_request {
            RuntimeRouteKind::Compact
        } else {
            RuntimeRouteKind::Standard
        },
    )?;
    if compact_request {
        remember_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            response_turn_state.as_deref(),
            RuntimeRouteKind::Compact,
        )?;
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_committed_owner profile={profile_name} session={} turn_state={}",
                request_session_id.unwrap_or("-"),
                response_turn_state.as_deref().unwrap_or("-"),
            ),
        );
    }
    Ok(RuntimeStandardAttempt::Success {
        profile_name: profile_name.to_string(),
        response,
    })
}
