use super::*;

fn runtime_proxy_cookie_namespace(shared: &RuntimeRotationProxyShared) -> String {
    shared.log_path.display().to_string()
}

pub(crate) fn runtime_proxy_cookie_header_for_reqwest(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    upstream_url: &str,
    request_headers: &[(String, String)],
) -> Option<String> {
    let namespace = runtime_proxy_cookie_namespace(shared);
    prodex_runtime_cookies::runtime_proxy_cookie_header_for_reqwest_namespace(
        &namespace,
        profile_name,
        upstream_url,
        request_headers,
    )
}

pub(crate) fn runtime_proxy_capture_reqwest_cookies(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response: &reqwest::Response,
) {
    let namespace = runtime_proxy_cookie_namespace(shared);
    prodex_runtime_cookies::runtime_proxy_capture_reqwest_cookies_namespace(
        &namespace,
        profile_name,
        response,
    );
}

pub(crate) fn runtime_proxy_cookie_header_for_websocket(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    upstream_url: &str,
    request_headers: &[(String, String)],
) -> Option<String> {
    let namespace = runtime_proxy_cookie_namespace(shared);
    prodex_runtime_cookies::runtime_proxy_cookie_header_for_websocket_namespace(
        &namespace,
        profile_name,
        upstream_url,
        request_headers,
    )
}

pub(crate) fn runtime_proxy_capture_websocket_cookies(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    upstream_url: &str,
    headers: &tungstenite::http::HeaderMap,
) {
    let namespace = runtime_proxy_cookie_namespace(shared);
    prodex_runtime_cookies::runtime_proxy_capture_websocket_cookies_namespace(
        &namespace,
        profile_name,
        upstream_url,
        headers,
    );
}
