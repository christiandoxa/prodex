use super::build_runtime_proxy_json_error_response;

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_request_timeout_response()
-> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        504,
        "request_timeout",
        "gateway request deadline exceeded",
    )
}
