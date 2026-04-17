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
    } else {
        noncompact::proxy_runtime_noncompact_request(request_id, request, shared)
    }
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
