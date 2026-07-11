use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_boundary::RuntimeGatewayAdminPreauthorization;
use super::local_rewrite_gateway_admin_audit::runtime_gateway_audit_admin_request_denied_event;
use super::local_rewrite_gateway_admin_route_explain::RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_BODY_BYTES;
use super::local_rewrite_gateway_admin_router::runtime_gateway_admin_response;
use super::{
    build_runtime_proxy_json_error_response, capture_runtime_proxy_request_with_limit,
    path_without_query, runtime_proxy_error_is_body_too_large,
};

pub(super) fn runtime_gateway_respond_route_explain(
    mut request: tiny_http::Request,
    request_path: &str,
    shared: &RuntimeLocalRewriteProxyShared,
    request_context: prodex_application::ApplicationRequestContext<'_>,
    preauthorized_admin: Option<RuntimeGatewayAdminPreauthorization<'_>>,
) {
    let mut captured = match capture_runtime_proxy_request_with_limit(
        &mut request,
        shared.runtime_shared.runtime_config.max_request_body_bytes,
        RUNTIME_GATEWAY_ROUTE_EXPLAIN_MAX_BODY_BYTES as u64,
    ) {
        Ok(captured) => captured,
        Err(err) => {
            let (status, code, message) = if runtime_proxy_error_is_body_too_large(&err) {
                (
                    413,
                    "request_body_too_large",
                    "gateway admin request body is too large",
                )
            } else {
                (
                    400,
                    "request_capture_failed",
                    "gateway admin request could not be read",
                )
            };
            if let Some(admin_auth) = preauthorized_admin.as_ref() {
                runtime_gateway_audit_admin_request_denied_event(
                    shared,
                    &admin_auth.auth.name,
                    admin_auth.auth.role.as_str(),
                    code,
                    request.method().as_str(),
                    path_without_query(request_path),
                );
            }
            let _ = request.respond(build_runtime_proxy_json_error_response(
                status, code, message,
            ));
            return;
        }
    };
    captured.path_and_query = request_context.target().path_and_query().to_string();
    let response =
        runtime_gateway_admin_response(0, &captured, shared, request_context, preauthorized_admin)
            .unwrap_or_else(|| {
                build_runtime_proxy_json_error_response(
                    404,
                    "admin_route_not_found",
                    "gateway admin route was not found",
                )
            });
    let _ = request.respond(response);
}
