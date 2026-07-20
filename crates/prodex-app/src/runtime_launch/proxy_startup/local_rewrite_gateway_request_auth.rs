use super::local_rewrite_request::RuntimeLocalRewriteRequest;

pub(super) fn runtime_local_rewrite_request_is_authorized(
    request: &RuntimeLocalRewriteRequest,
    auth_token_hash: &runtime_proxy_crate::LocalBridgeBearerTokenHash,
) -> bool {
    let path = runtime_proxy_crate::path_without_query(request.url());
    if path == runtime_proxy_crate::LOCAL_BRIDGE_HEALTH_PATH
        || path.ends_with("/prodex/gateway/admin")
    {
        return true;
    }
    request.headers().iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("authorization")
            && auth_token_hash.verify_authorization_header(value)
    })
}
