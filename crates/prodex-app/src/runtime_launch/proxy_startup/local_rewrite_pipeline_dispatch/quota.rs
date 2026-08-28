use super::super::super::local_rewrite_application_data_plane::{
    RuntimeGatewayTenantResource, runtime_gateway_application_http_policy,
    runtime_gateway_application_trace_context,
};
use super::super::super::local_rewrite_gateway_admin_router::runtime_gateway_http_request_meta;
use super::super::{
    RuntimeLocalRewriteCapturedRequest, RuntimeLocalRewritePipelineResult,
    RuntimeLocalRewriteProxyShared, build_runtime_proxy_json_error_response,
    runtime_local_rewrite_request_timeout_response,
};
use crate::runtime_proxy::{
    RuntimeHeapTrimmedBufferedResponseParts, build_runtime_proxy_response_from_parts,
    runtime_profile_usage_cache_is_fresh, runtime_usage_snapshot_is_usable,
};
use prodex_application::{
    ApplicationQuotaReadError, ApplicationQuotaReadRequest, plan_application_quota_read,
    plan_application_quota_read_error_response,
};
use prodex_gateway_core::GatewayQuotaReadAuthorizationRequest;
use prodex_gateway_http::{GatewayHttpErrorStatus, GatewayHttpRouteKind};
use prodex_quota::UsageResponse;

pub(crate) fn runtime_local_rewrite_dispatch_quota<'target>(
    request: RuntimeLocalRewriteCapturedRequest<'target>,
    shared: &RuntimeLocalRewriteProxyShared,
) -> RuntimeLocalRewritePipelineResult<RuntimeLocalRewriteCapturedRequest<'target>> {
    if request.state.context.route() != GatewayHttpRouteKind::DataPlaneQuota {
        return Ok(request);
    }
    if request.state.deadline_expired() {
        return Err(request
            .state
            .reject(runtime_local_rewrite_request_timeout_response()));
    }

    let Some(authorized) = request.state.application.as_ref() else {
        return Err(request
            .state
            .reject(build_runtime_proxy_json_error_response(
                503,
                "gateway_quota_authorization_unavailable",
                "gateway quota authorization is temporarily unavailable",
            )));
    };
    let Some(tenant) = authorized.tenant_context() else {
        return Err(request
            .state
            .reject(build_runtime_proxy_json_error_response(
                401,
                "missing_or_invalid_token",
                "missing or invalid gateway bearer token",
            )));
    };
    let Some(principal) = authorized.principal().cloned() else {
        return Err(request
            .state
            .reject(build_runtime_proxy_json_error_response(
                401,
                "missing_or_invalid_token",
                "missing or invalid gateway bearer token",
            )));
    };
    let trace_context = match authorized.request().trace_context().cloned() {
        Some(trace_context) => trace_context,
        None => {
            match runtime_gateway_application_trace_context(authorized.request().request_id()) {
                Ok(trace_context) => trace_context,
                Err(_) => {
                    return Err(request
                        .state
                        .reject(build_runtime_proxy_json_error_response(
                            503,
                            "gateway_quota_authorization_unavailable",
                            "gateway quota authorization is temporarily unavailable",
                        )));
                }
            }
        }
    };

    let plan = plan_application_quota_read(
        runtime_gateway_application_http_policy(shared),
        ApplicationQuotaReadRequest {
            http: runtime_gateway_http_request_meta(
                &request.captured,
                request.captured.path_and_query.as_str(),
            ),
            authorization: GatewayQuotaReadAuthorizationRequest {
                tenant,
                principal,
                resource: RuntimeGatewayTenantResource {
                    tenant_id: tenant.tenant_id,
                },
                request_id: authorized.request().request_id(),
                trace_context,
            },
        },
    );
    if let Err(error) = plan {
        return Err(request
            .state
            .reject(runtime_gateway_quota_plan_error_response(&error)));
    }

    let Some(usage) = runtime_gateway_quota_usage(shared) else {
        return Err(request
            .state
            .reject(runtime_gateway_quota_unavailable_response()));
    };
    Err(request
        .state
        .respond(runtime_gateway_quota_success_response(usage)))
}

fn runtime_gateway_quota_usage(shared: &RuntimeLocalRewriteProxyShared) -> Option<UsageResponse> {
    let now = chrono::Local::now().timestamp();
    let runtime = shared.runtime_shared.lock_runtime_state().ok()?;
    if let Some(entry) = runtime.profile_probe_cache.get(&runtime.current_profile)
        && runtime_profile_usage_cache_is_fresh(entry, now)
        && let Ok(usage) = &entry.result
    {
        return Some(usage.clone());
    }
    runtime
        .profile_usage_snapshots
        .get(&runtime.current_profile)
        .filter(|snapshot| runtime_usage_snapshot_is_usable(snapshot, now))
        .map(prodex_runtime_quota::usage_from_runtime_usage_snapshot)
}

fn runtime_gateway_quota_success_response(usage: UsageResponse) -> tiny_http::ResponseBox {
    let body = match serde_json::to_vec(&usage) {
        Ok(body) => body,
        Err(_) => {
            return build_runtime_proxy_json_error_response(
                500,
                "gateway_quota_response_failed",
                "gateway quota response could not be serialized",
            );
        }
    };
    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![
            ("content-type".to_string(), b"application/json".to_vec()),
            ("cache-control".to_string(), b"no-store".to_vec()),
            ("x-content-type-options".to_string(), b"nosniff".to_vec()),
        ],
        body: body.into(),
    })
}

fn runtime_gateway_quota_unavailable_response() -> tiny_http::ResponseBox {
    build_runtime_proxy_json_error_response(
        503,
        "gateway_quota_unavailable",
        "gateway quota data is temporarily unavailable",
    )
}

fn runtime_gateway_quota_plan_error_response(
    error: &ApplicationQuotaReadError,
) -> tiny_http::ResponseBox {
    let response = plan_application_quota_read_error_response(error);
    if let Some(http) = response.http {
        let status = match http.status {
            GatewayHttpErrorStatus::BadRequest => 400,
            GatewayHttpErrorStatus::MethodNotAllowed => 405,
            GatewayHttpErrorStatus::PayloadTooLarge => 413,
            GatewayHttpErrorStatus::RequestHeaderFieldsTooLarge => 431,
            GatewayHttpErrorStatus::InternalServerError => 500,
        };
        return build_runtime_proxy_json_error_response(status, http.code, http.message);
    }
    if let Some(authorization) = response.authorization {
        let status = match authorization.status {
            prodex_gateway_core::GatewayQuotaReadAuthorizationErrorStatus::BadRequest => 400,
            prodex_gateway_core::GatewayQuotaReadAuthorizationErrorStatus::Forbidden => 403,
            prodex_gateway_core::GatewayQuotaReadAuthorizationErrorStatus::ServiceUnavailable => {
                503
            }
        };
        return build_runtime_proxy_json_error_response(
            status,
            authorization.code,
            authorization.message,
        );
    }
    build_runtime_proxy_json_error_response(
        503,
        "gateway_quota_authorization_unavailable",
        "gateway quota authorization is temporarily unavailable",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_quota::{UsageWindow, WindowPair};
    use std::io::Read;

    fn quota_fixture() -> UsageResponse {
        UsageResponse {
            email: None,
            plan_type: Some("plus".to_string()),
            rate_limit: Some(WindowPair {
                allowed: None,
                limit_reached: None,
                extra: std::collections::BTreeMap::new(),
                primary_window: Some(UsageWindow {
                    used_percent: Some(20),
                    reset_at: Some(1_700_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            rate_limit_reset_credits: None,
            additional_rate_limits: Vec::new(),
        }
    }

    #[test]
    fn quota_success_serializes_the_existing_usage_contract() {
        let expected = quota_fixture();
        let response = runtime_gateway_quota_success_response(expected.clone());
        assert_eq!(response.status_code().0, 200);

        let mut body = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut body)
            .expect("quota response body should be readable");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).expect("quota JSON should parse"),
            serde_json::to_value(expected).expect("quota fixture should serialize")
        );
    }

    #[test]
    fn quota_unavailable_response_is_stable() {
        let response = runtime_gateway_quota_unavailable_response();
        assert_eq!(response.status_code().0, 503);
        let mut body = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut body)
            .expect("quota error body should be readable");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).expect("quota error should parse"),
            serde_json::json!({
                "error": {
                    "code": "gateway_quota_unavailable",
                    "message": "gateway quota data is temporarily unavailable"
                }
            })
        );
    }
}
