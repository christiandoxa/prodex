use super::*;
use prodex_application::ApplicationRequestContextError;

pub(super) fn runtime_local_rewrite_request_timeout_response() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        504,
        "request_timeout",
        "gateway request deadline exceeded",
    )
}

pub(super) fn runtime_local_rewrite_application_context_rejection(
    error: ApplicationRequestContextError,
) -> tiny_http::ResponseBox {
    let ApplicationRequestContextError::Trace(error) = error else {
        return build_runtime_proxy_json_error_response(
            404,
            "route_not_available",
            "route is not available",
        );
    };
    let response = prodex_gateway_http::plan_gateway_http_error_response(&error);
    let status = match response.status {
        prodex_gateway_http::GatewayHttpErrorStatus::BadRequest => 400,
        prodex_gateway_http::GatewayHttpErrorStatus::MethodNotAllowed => 405,
        prodex_gateway_http::GatewayHttpErrorStatus::PayloadTooLarge => 413,
        prodex_gateway_http::GatewayHttpErrorStatus::RequestHeaderFieldsTooLarge => 431,
        prodex_gateway_http::GatewayHttpErrorStatus::InternalServerError => 500,
    };
    build_runtime_proxy_json_error_response(status, response.code, response.message)
}
