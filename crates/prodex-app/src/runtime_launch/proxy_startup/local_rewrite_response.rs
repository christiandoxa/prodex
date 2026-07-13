use self::dispatch::respond_runtime_local_rewrite_live_response;
use super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, runtime_gateway_guardrail_webhook_block,
};
use super::local_rewrite_request::RuntimeLocalRewriteRequest;
use super::local_rewrite_response_guardrails::{
    RuntimeGatewayGuardrailStreamPlan, runtime_gateway_guardrail_stream_body,
};
use super::local_rewrite_response_spend::{
    emit_runtime_gateway_response_spend_event_for_body, runtime_gateway_spend_stream_body,
};
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, RuntimeStreamingResponse,
    build_runtime_proxy_json_error_response, build_runtime_proxy_response_from_parts,
    build_runtime_proxy_text_response, runtime_proxy_log,
};
use anyhow::{Context, Result};
use prodex_application::ApplicationResponseObligationPlan;
use prodex_domain::CallId;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::Read;
use std::path::Path;

#[path = "local_rewrite_response_dispatch.rs"]
mod dispatch;
#[path = "local_rewrite_response_chat_compatible.rs"]
mod local_rewrite_response_chat_compatible;
#[path = "local_rewrite_response_copilot.rs"]
mod local_rewrite_response_copilot;
#[path = "local_rewrite_response_gemini.rs"]
mod local_rewrite_response_gemini;
#[path = "local_rewrite_response_passthrough.rs"]
mod local_rewrite_response_passthrough;

fn runtime_local_rewrite_invalid_response(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    _error: &dyn std::fmt::Display,
) -> tiny_http::ResponseBox {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "provider_response_translation_failed",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("error", "translation_failed"),
            ],
        ),
    );
    build_runtime_proxy_text_response(502, "provider response could not be processed")
}

pub(super) fn runtime_local_rewrite_buffered_response_from_response(
    response: reqwest::blocking::Response,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let status = response.status().as_u16();
    let headers = runtime_proxy_crate::runtime_forward_binary_response_headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_bytes())),
    );
    runtime_local_rewrite_buffered_response_parts(status, headers, response)
}

fn runtime_gateway_audit_response_blocked(
    shared: &RuntimeLocalRewriteProxyShared,
    action: &str,
    reason: &str,
) {
    let default_log_dir = shared
        .runtime_shared
        .log_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let path = prodex_audit_log::audit_log_path(default_log_dir);
    let _ = prodex_audit_log::append_audit_event(
        &path,
        "gateway_data_plane",
        action,
        "failure",
        serde_json::json!({
            "state_backend": shared.gateway_state_store.label(),
            "details": {"reason": reason},
        }),
    );
}

pub(super) fn runtime_local_rewrite_response_with_call_id(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
) -> tiny_http::ResponseBox {
    runtime_local_rewrite_governed_response_with_call_id(parts, request_id, shared, None)
}

pub(super) fn runtime_local_rewrite_governed_response_with_call_id(
    mut parts: RuntimeHeapTrimmedBufferedResponseParts,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    obligations: Option<ApplicationResponseObligationPlan>,
) -> tiny_http::ResponseBox {
    if (200..300).contains(&parts.status)
        && obligations.is_some_and(|plan| plan.inspection_required)
    {
        match crate::runtime_proxy::presidio::local::runtime_local_inspect_and_mask(
            parts.body.as_slice().to_vec(),
        ) {
            Ok(inspected) => {
                if obligations.is_some_and(|plan| {
                    plan.enforce
                        && plan.require_full_inspection
                        && inspected.coverage != prodex_domain::InspectionCoverage::Full
                }) {
                    runtime_gateway_audit_response_blocked(
                        shared,
                        "response_inspection_failed",
                        "response_inspection_incomplete",
                    );
                    return build_runtime_proxy_json_error_response(
                        403,
                        "response_inspection_incomplete",
                        "gateway response policy denied this response",
                    );
                }
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "gateway_response_inspection",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field("coverage", inspected.coverage.as_str()),
                            runtime_proxy_log_field(
                                "finding_count",
                                inspected.findings.len().to_string(),
                            ),
                            runtime_proxy_log_field("changed", inspected.changed.to_string()),
                        ],
                    ),
                );
                parts.body = inspected.body.into();
            }
            Err(_) if obligations.is_some_and(|plan| plan.enforce) => {
                runtime_gateway_audit_response_blocked(
                    shared,
                    "response_inspection_failed",
                    "response_inspection_unsupported",
                );
                return build_runtime_proxy_json_error_response(
                    403,
                    "response_inspection_unsupported",
                    "gateway response policy denied this response",
                );
            }
            Err(_) => {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "gateway_response_inspection",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("transport", "http"),
                            runtime_proxy_log_field("coverage", "unsupported"),
                            runtime_proxy_log_field("finding_count", "0"),
                            runtime_proxy_log_field("changed", "false"),
                        ],
                    ),
                );
            }
        }
    }
    if (200..300).contains(&parts.status)
        && obligations.is_some_and(|plan| {
            plan.enforce
                && plan.maximum_output_tokens.is_some_and(|limit| {
                    u32::try_from(parts.body.len().saturating_add(3) / 4).unwrap_or(u32::MAX)
                        > limit
                })
        })
    {
        runtime_gateway_audit_response_blocked(
            shared,
            "response_obligation_blocked",
            "output_token_limit_exceeded",
        );
        return build_runtime_proxy_json_error_response(
            403,
            "output_token_limit_exceeded",
            "gateway response policy denied this response",
        );
    }
    if (200..300).contains(&parts.status)
        && let Some(block) = runtime_proxy_crate::runtime_gateway_response_guardrail_block(
            &parts.body,
            &shared.gateway_guardrails,
        )
    {
        runtime_gateway_audit_response_blocked(
            shared,
            "response_guardrail_blocked",
            block.kind.as_str(),
        );
        crate::runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_response_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("reason", block.kind.as_str()),
                    runtime_proxy_log_field("matched_value_redacted", "true"),
                ],
            ),
        );
        return build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail blocked this response",
        );
    }
    if (200..300).contains(&parts.status)
        && let Some(block) =
            runtime_gateway_guardrail_webhook_block("post", request_id, &parts.body, shared)
    {
        runtime_gateway_audit_response_blocked(
            shared,
            "response_guardrail_webhook_blocked",
            block.reason.as_str(),
        );
        crate::runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_webhook_blocked",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http"),
                    runtime_proxy_log_field("phase", "post"),
                    runtime_proxy_log_field("reason", block.reason.as_str()),
                    runtime_proxy_log_field("matched_value_redacted", "true"),
                ],
            ),
        );
        return build_runtime_proxy_json_error_response(
            403,
            "policy_violation",
            "gateway guardrail webhook blocked this response",
        );
    }
    if let Some(header_name) = shared.gateway_call_id_header.as_deref() {
        parts.headers.push((
            header_name.to_string(),
            runtime_local_rewrite_call_id(request_id, shared).into_bytes(),
        ));
    }
    build_runtime_proxy_response_from_parts(parts)
}

pub(super) fn respond_runtime_local_rewrite_stream(
    request: RuntimeLocalRewriteRequest,
    mut streaming: RuntimeStreamingResponse,
    shared: &RuntimeLocalRewriteProxyShared,
    obligations: Option<ApplicationResponseObligationPlan>,
) {
    let body = std::mem::replace(&mut streaming.body, Box::new(std::io::empty()));
    match runtime_gateway_guardrail_stream_body(body, streaming.request_id, shared, obligations) {
        Ok(RuntimeGatewayGuardrailStreamPlan::Allowed(body)) => {
            streaming.body = body;
            let _ = request.stream(streaming, None);
        }
        Ok(RuntimeGatewayGuardrailStreamPlan::Blocked(reason)) => {
            let _ = request.respond(build_runtime_proxy_json_error_response(
                403,
                reason,
                "gateway guardrail blocked this response",
            ));
        }
        Err(error) => {
            let request_id = streaming.request_id;
            let _ = request.respond(runtime_local_rewrite_invalid_response(
                request_id, shared, &error,
            ));
        }
    }
}

pub(super) fn respond_runtime_local_rewrite_proxy_request(
    request_id: u64,
    request: RuntimeLocalRewriteRequest,
    response: RuntimeLocalRewriteUpstreamResult,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    obligations: Option<ApplicationResponseObligationPlan>,
) {
    let RuntimeLocalRewriteUpstreamResult {
        response,
        gemini_context,
        copilot_context,
    } = response;
    match response {
        RuntimeLocalRewriteUpstreamResponse::Streaming(streaming_response) => {
            let mut headers = streaming_response.headers;
            runtime_local_rewrite_append_call_id_header(&mut headers, request_id, shared);
            let body = runtime_gateway_spend_stream_body(
                streaming_response.body,
                request_id,
                streaming_response.status,
                captured,
                shared,
            );
            let streaming = RuntimeStreamingResponse {
                status: streaming_response.status,
                headers,
                body,
                request_id,
                profile_name: streaming_response.profile_name,
                log_path: shared.runtime_shared.log_path.clone(),
                shared: shared.runtime_shared.clone(),
                _inflight_guard: None,
            };
            respond_runtime_local_rewrite_stream(request, streaming, shared, obligations);
        }
        RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
            emit_runtime_gateway_response_spend_event_for_body(
                request_id,
                captured,
                shared,
                parts.status,
                0,
                parts.body.as_slice(),
            );
            let _ = request.respond(runtime_local_rewrite_governed_response_with_call_id(
                parts,
                request_id,
                shared,
                obligations,
            ));
        }
        RuntimeLocalRewriteUpstreamResponse::Live(live_response) => {
            respond_runtime_local_rewrite_live_response(
                request_id,
                request,
                live_response,
                gemini_context,
                copilot_context,
                captured,
                shared,
                obligations,
            );
        }
    }
}

fn runtime_local_rewrite_call_id(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
) -> String {
    shared
        .gateway_usage
        .call_ids
        .lock()
        .ok()
        .and_then(|call_ids| call_ids.get(&request_id).cloned())
        .unwrap_or_else(|| format!("prodex-{}", CallId::new()))
}

pub(super) fn runtime_local_rewrite_append_call_id_header(
    headers: &mut Vec<(String, String)>,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
) {
    if let Some(header_name) = shared.gateway_call_id_header.as_deref() {
        headers.push((
            header_name.to_string(),
            runtime_local_rewrite_call_id(request_id, shared),
        ));
    }
}

pub(super) fn runtime_local_rewrite_buffered_response_parts(
    status: u16,
    headers: Vec<(String, Vec<u8>)>,
    mut response: reqwest::blocking::Response,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read local provider response body")?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers,
        body: body.into(),
    })
}
