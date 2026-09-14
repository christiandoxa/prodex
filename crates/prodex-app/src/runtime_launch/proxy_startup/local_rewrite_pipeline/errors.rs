use super::build_runtime_proxy_json_error_response;
use prodex_application::ApplicationRequestContextError;

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_request_timeout_response()
-> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        504,
        "request_timeout",
        "gateway request deadline exceeded",
    )
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_local_rewrite_application_context_rejection(
    error: ApplicationRequestContextError,
) -> tiny_http::ResponseBox {
    #[cfg(feature = "mojo-core")]
    {
        runtime_local_rewrite_application_context_rejection_mojo(error)
    }

    #[cfg(not(feature = "mojo-core"))]
    {
        runtime_local_rewrite_application_context_rejection_rust(error)
    }
}

#[cfg(feature = "mojo-core")]
fn runtime_local_rewrite_application_context_rejection_mojo(
    error: ApplicationRequestContextError,
) -> tiny_http::ResponseBox {
    use prodex_mojo_core::rich::{
        ApplicationGatewayErrorStatus as MojoStatus, ApplicationPipelineErrorInput as MojoInput,
    };
    let (input, response) = match error {
        ApplicationRequestContextError::UnknownRoute => (MojoInput::UnknownRoute, None),
        ApplicationRequestContextError::Trace(error) => {
            let response = prodex_gateway_http::plan_gateway_http_error_response(&error);
            let status = match response.status {
                prodex_gateway_http::GatewayHttpErrorStatus::BadRequest => MojoStatus::BadRequest,
                prodex_gateway_http::GatewayHttpErrorStatus::MethodNotAllowed => {
                    MojoStatus::MethodNotAllowed
                }
                prodex_gateway_http::GatewayHttpErrorStatus::PayloadTooLarge => {
                    MojoStatus::PayloadTooLarge
                }
                prodex_gateway_http::GatewayHttpErrorStatus::RequestHeaderFieldsTooLarge => {
                    MojoStatus::RequestHeaderFieldsTooLarge
                }
                prodex_gateway_http::GatewayHttpErrorStatus::InternalServerError => {
                    MojoStatus::InternalServerError
                }
            };
            (MojoInput::Gateway(status), Some(response))
        }
    };
    let plan = prodex_mojo_core::rich::plan_application_pipeline_error(input)
        .expect("Mojo application pipeline error plan returned invalid output");
    match response {
        Some(response) if plan.gateway_response => {
            build_runtime_proxy_json_error_response(plan.status, response.code, response.message)
        }
        None if !plan.gateway_response => build_runtime_proxy_json_error_response(
            plan.status,
            "route_not_available",
            "route is not available",
        ),
        _ => unreachable!("Mojo application pipeline error response kind must match its input"),
    }
}

#[cfg(not(feature = "mojo-core"))]
fn runtime_local_rewrite_application_context_rejection_rust(
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
