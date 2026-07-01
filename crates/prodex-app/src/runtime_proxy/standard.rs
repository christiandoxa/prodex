use super::*;

mod attempts;
mod compact;
mod noncompact;

pub(crate) fn proxy_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Result<tiny_http::ResponseBox> {
    if is_runtime_compact_path(&request.path_and_query) {
        compact::proxy_runtime_compact_request(request_id, request, shared)
    } else if let Some(response) =
        runtime_startup_metadata_pressure_response(request_id, request, shared)
    {
        Ok(response)
    } else {
        noncompact::proxy_runtime_noncompact_request(request_id, request, shared)
    }
}

pub(crate) fn runtime_startup_metadata_admission_pressure_response(
    request_id: u64,
    path_and_query: &str,
    shared: &RuntimeRotationProxyShared,
) -> Option<tiny_http::ResponseBox> {
    if !runtime_proxy_should_shed_startup_metadata(shared, path_and_query) {
        return None;
    }
    runtime_startup_metadata_response_for_path(request_id, path_and_query, shared)
}

fn runtime_startup_metadata_pressure_response(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
) -> Option<tiny_http::ResponseBox> {
    if !runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Standard) {
        return None;
    }
    runtime_startup_metadata_response_for_path(request_id, &request.path_and_query, shared)
}

fn runtime_proxy_should_shed_startup_metadata(
    shared: &RuntimeRotationProxyShared,
    path_and_query: &str,
) -> bool {
    runtime_proxy_should_shed_startup_metadata_for_counts(
        path_and_query,
        runtime_proxy_pressure_mode_active_for_route(shared, RuntimeRouteKind::Standard),
        shared
            .lane_admission
            .active_counter(RuntimeRouteKind::Standard)
            .load(Ordering::SeqCst),
        shared.lane_admission.limit(RuntimeRouteKind::Standard),
    )
}

fn runtime_proxy_should_shed_startup_metadata_for_counts(
    path_and_query: &str,
    pressure_mode: bool,
    active: usize,
    limit: usize,
) -> bool {
    if runtime_proxy_startup_metadata_response_body(path_and_query).is_none() {
        return false;
    }
    pressure_mode || active >= (limit.max(1) / 2).max(1)
}

pub(crate) fn runtime_proxy_startup_standard_lane_priority_path(path_and_query: &str) -> bool {
    let normalized = runtime_proxy_normalize_openai_path(path_and_query);
    matches!(
        path_without_query(normalized.as_ref()),
        "/backend-api/codex/models" | "/backend-api/ps/mcp"
    )
}

fn runtime_startup_metadata_response_for_path(
    request_id: u64,
    path_and_query: &str,
    shared: &RuntimeRotationProxyShared,
) -> Option<tiny_http::ResponseBox> {
    let body = runtime_proxy_startup_metadata_response_body(path_and_query)?;
    match body {
        RuntimeStartupMetadataResponseBody::NoContent => {
            runtime_proxy_log_startup_metadata_shed(request_id, path_and_query, shared, 204);
            Some(build_runtime_proxy_text_response(204, ""))
        }
        RuntimeStartupMetadataResponseBody::Json(body) => {
            runtime_proxy_log_startup_metadata_shed(request_id, path_and_query, shared, 200);
            Some(runtime_proxy_json_response(200, body))
        }
    }
}

enum RuntimeStartupMetadataResponseBody {
    NoContent,
    Json(&'static str),
}

fn runtime_proxy_startup_metadata_response_body(
    path_and_query: &str,
) -> Option<RuntimeStartupMetadataResponseBody> {
    let normalized = runtime_proxy_normalize_openai_path(path_and_query);
    match path_without_query(normalized.as_ref()) {
        "/backend-api/codex/analytics-events/events" => {
            Some(RuntimeStartupMetadataResponseBody::NoContent)
        }
        "/backend-api/plugins/featured"
        | "/backend-api/ps/plugins/installed"
        | "/backend-api/connectors/directory/list" => Some(
            RuntimeStartupMetadataResponseBody::Json(r#"{"items":[],"data":[]}"#),
        ),
        _ => None,
    }
}

fn runtime_proxy_log_startup_metadata_shed(
    request_id: u64,
    path_and_query: &str,
    shared: &RuntimeRotationProxyShared,
    status: u16,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "startup_metadata_pressure_response",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", "http"),
                runtime_proxy_log_field("path", path_and_query),
                runtime_proxy_log_field("status", status.to_string()),
            ],
        ),
    );
}

fn runtime_proxy_json_response(status: u16, body: &str) -> tiny_http::ResponseBox {
    let mut response = TinyResponse::from_string(body.to_string())
        .with_status_code(TinyStatusCode(status))
        .boxed();
    if let Ok(header) = TinyHeader::from_bytes("Content-Type", "application/json") {
        response = response.with_header(header);
    }
    response
}

pub(crate) fn attempt_runtime_noncompact_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
) -> Result<RuntimeStandardAttempt> {
    attempts::attempt_runtime_noncompact_standard_request(request_id, request, shared, profile_name)
}

pub(crate) fn attempt_runtime_noncompact_standard_request_with_policy(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    enforce_local_precommit_quota_guard: bool,
) -> Result<RuntimeStandardAttempt> {
    attempts::attempt_runtime_noncompact_standard_request_with_policy(
        request_id,
        request,
        shared,
        profile_name,
        enforce_local_precommit_quota_guard,
    )
}

pub(crate) fn attempt_runtime_standard_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    allow_quota_exhausted_send: bool,
) -> Result<RuntimeStandardAttempt> {
    attempts::attempt_runtime_standard_request(
        request_id,
        request,
        shared,
        profile_name,
        allow_quota_exhausted_send,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_priority_paths_bypass_standard_lane_limit() {
        assert!(runtime_proxy_startup_standard_lane_priority_path(
            "/backend-api/prodex/models?client_version=0.142.5"
        ));
        assert!(runtime_proxy_startup_standard_lane_priority_path(
            "/backend-api/ps/mcp"
        ));
        assert!(!runtime_proxy_startup_standard_lane_priority_path(
            "/backend-api/plugins/featured?platform=codex"
        ));
    }

    #[test]
    fn startup_metadata_pressure_response_only_covers_optional_metadata() {
        assert!(matches!(
            runtime_proxy_startup_metadata_response_body(
                "/backend-api/codex/analytics-events/events"
            ),
            Some(RuntimeStartupMetadataResponseBody::NoContent)
        ));
        assert!(matches!(
            runtime_proxy_startup_metadata_response_body(
                "/backend-api/plugins/featured?platform=codex"
            ),
            Some(RuntimeStartupMetadataResponseBody::Json(_))
        ));
        assert!(runtime_proxy_startup_metadata_response_body("/backend-api/ps/mcp").is_none());
        assert!(
            runtime_proxy_startup_metadata_response_body("/backend-api/prodex/models").is_none()
        );
    }

    #[test]
    fn optional_startup_metadata_sheds_before_standard_lane_is_full() {
        let path = "/backend-api/plugins/featured?platform=codex";
        assert!(!runtime_proxy_should_shed_startup_metadata_for_counts(
            path, false, 5, 12
        ));
        assert!(runtime_proxy_should_shed_startup_metadata_for_counts(
            path, false, 6, 12
        ));
        assert!(runtime_proxy_should_shed_startup_metadata_for_counts(
            path, true, 0, 12
        ));
        assert!(!runtime_proxy_should_shed_startup_metadata_for_counts(
            "/backend-api/ps/mcp",
            true,
            12,
            12
        ));
    }
}
