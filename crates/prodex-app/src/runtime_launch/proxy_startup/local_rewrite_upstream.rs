use super::deepseek_rewrite::RuntimeDeepSeekPendingRequest;
use super::local_rewrite::{RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_anthropic::send_runtime_anthropic_upstream_request;
use super::local_rewrite_application_data_plane::RuntimeGatewayApplicationProviderDispatch;
use super::local_rewrite_copilot::{
    RuntimeCopilotRequestContext, send_runtime_copilot_upstream_request,
};
use super::local_rewrite_deepseek::send_runtime_deepseek_upstream_request;
use super::local_rewrite_gemini::{
    RuntimeGeminiRequestContext, send_runtime_gemini_upstream_request,
};
use super::local_rewrite_kiro::send_runtime_kiro_upstream_request;
use super::local_rewrite_model_memory::runtime_local_rewrite_model_selection;
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_local_rewrite_api_key_attempts,
    runtime_local_rewrite_upstream_url, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeHarnessProviderPolicyLog, RuntimeProviderBridgeKind,
    runtime_harness_log_provider_policy, runtime_provider_model_from_body,
};
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, RuntimeRouteKind,
    prepare_runtime_smart_context_http_body, runtime_proxy_log,
};
use anyhow::Result;
use prodex_provider_core::{ProviderEndpoint, ProviderId};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use serde_json::{Value, json};

pub(super) struct RuntimeLocalRewriteUpstreamResult {
    pub(super) response: RuntimeLocalRewriteUpstreamResponse,
    pub(super) gemini_context: Option<RuntimeGeminiRequestContext>,
    pub(super) copilot_context: Option<RuntimeCopilotRequestContext>,
}

impl RuntimeLocalRewriteUpstreamResult {
    pub(super) fn status(&self) -> u16 {
        match &self.response {
            RuntimeLocalRewriteUpstreamResponse::Live(live) => live.response.status().as_u16(),
            RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => parts.status,
            RuntimeLocalRewriteUpstreamResponse::Streaming(streaming) => streaming.status,
        }
    }
}

pub(super) enum RuntimeLocalRewriteUpstreamResponse {
    Live(RuntimeLocalRewriteLiveResponse),
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
    Streaming(RuntimeLocalRewriteStreamingResponse),
}

pub(super) struct RuntimeLocalRewriteLiveResponse {
    pub(super) response: reqwest::blocking::Response,
    pub(super) prefix: Vec<u8>,
    pub(super) native_anthropic_messages: bool,
    pub(super) chat_compatible_request: Option<RuntimeDeepSeekPendingRequest>,
}

pub(super) struct RuntimeLocalRewriteStreamingResponse {
    pub(super) status: u16,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Box<dyn std::io::Read + Send>,
    pub(super) profile_name: String,
}

impl RuntimeLocalRewriteLiveResponse {
    pub(super) fn new(response: reqwest::blocking::Response) -> Self {
        Self {
            response,
            prefix: Vec::new(),
            native_anthropic_messages: false,
            chat_compatible_request: None,
        }
    }

    pub(super) fn with_prefix(response: reqwest::blocking::Response, prefix: Vec<u8>) -> Self {
        Self {
            response,
            prefix,
            native_anthropic_messages: false,
            chat_compatible_request: None,
        }
    }

    pub(super) fn with_native_anthropic_messages(response: reqwest::blocking::Response) -> Self {
        Self {
            response,
            prefix: Vec::new(),
            native_anthropic_messages: true,
            chat_compatible_request: None,
        }
    }

    pub(super) fn with_chat_compatible_request(
        mut self,
        request: RuntimeDeepSeekPendingRequest,
    ) -> Self {
        self.chat_compatible_request = Some(request);
        self
    }
}

pub(super) fn send_runtime_local_rewrite_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    dispatch: &RuntimeGatewayApplicationProviderDispatch<'_>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let provider = dispatch.provider();
    let endpoint = dispatch.endpoint();
    let stream_mode = dispatch.stream_mode();
    let inspection = dispatch.inspection();
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "gateway_provider_dispatch",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", provider.label()),
                runtime_proxy_log_field(
                    "classification",
                    inspection.result.classification().as_str(),
                ),
                runtime_proxy_log_field("coverage", inspection.result.coverage().as_str()),
                runtime_proxy_log_field(
                    "finding_count",
                    inspection.result.findings().len().to_string(),
                ),
            ],
        ),
    );
    let route_kind = runtime_local_rewrite_route_kind(endpoint);
    let body = prepare_runtime_smart_context_http_body(
        request_id,
        request,
        &shared.runtime_shared,
        route_kind,
    )?
    .into_owned();
    let body = match runtime_harness_shape_request(
        request_id, request, shared, provider, endpoint, body,
    ) {
        Ok(body) => body,
        Err(parts) => {
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            });
        }
    };
    match (provider, shared.provider.as_ref()) {
        (_, RuntimeLocalRewriteProviderOptions::ProjectedCredential { .. }) => {
            unreachable!("projected provider wrapper must be split before dispatch")
        }
        (ProviderId::Anthropic, RuntimeLocalRewriteProviderOptions::Anthropic { auth }) => {
            send_runtime_anthropic_upstream_request(
                request_id, request, shared, body, auth, endpoint,
            )
        }
        (ProviderId::Copilot, RuntimeLocalRewriteProviderOptions::Copilot { auth }) => {
            send_runtime_copilot_upstream_request(request_id, request, shared, body, auth, endpoint)
        }
        (ProviderId::OpenAi, RuntimeLocalRewriteProviderOptions::OpenAiResponses { api_keys }) => {
            let upstream_url = runtime_local_rewrite_upstream_url(
                &shared.upstream_base_url,
                &shared.mount_path,
                &request.path_and_query,
            );
            let body = if endpoint == ProviderEndpoint::Responses {
                runtime_local_rewrite_model_selection(
                    shared,
                    RuntimeProviderBridgeKind::OpenAiResponses,
                    request,
                    &body,
                    "",
                )
                .body
            } else {
                body
            };
            let prepared_auth = if shared.provider_credential.is_some() {
                RuntimeLocalRewritePreparedAuth::OpenAiProjected
            } else {
                let auth_attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys);
                RuntimeLocalRewritePreparedAuth::OpenAiResponses {
                    api_key: auth_attempts.first().map(|(_, api_key)| *api_key),
                }
            };
            let response = send_runtime_local_rewrite_prepared_request(
                request_id,
                request,
                shared,
                &upstream_url,
                body,
                prepared_auth,
            )?;
            if response.status().as_u16() >= 400 {
                return Ok(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Buffered(
                        runtime_local_rewrite_buffered_response_from_response(response)?,
                    ),
                    gemini_context: None,
                    copilot_context: None,
                });
            }
            Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Live(
                    RuntimeLocalRewriteLiveResponse::new(response),
                ),
                gemini_context: None,
                copilot_context: None,
            })
        }
        (ProviderId::DeepSeek, RuntimeLocalRewriteProviderOptions::DeepSeek { api_keys, .. }) => {
            send_runtime_deepseek_upstream_request(
                request_id, request, shared, body, api_keys, endpoint,
            )
        }
        (ProviderId::Gemini, RuntimeLocalRewriteProviderOptions::Gemini { auth, .. }) => {
            send_runtime_gemini_upstream_request(
                request_id,
                request,
                shared,
                body,
                auth,
                endpoint,
                stream_mode,
            )
        }
        (ProviderId::Kiro, RuntimeLocalRewriteProviderOptions::Kiro { auth }) => {
            send_runtime_kiro_upstream_request(
                request_id,
                request,
                shared,
                body,
                auth,
                endpoint,
                stream_mode,
            )
        }
        _ => anyhow::bail!("application provider dispatch does not match configured adapter"),
    }
}

fn runtime_harness_shape_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    body: Vec<u8>,
) -> std::result::Result<Vec<u8>, RuntimeHeapTrimmedBufferedResponseParts> {
    if endpoint != ProviderEndpoint::Responses {
        runtime_harness_log_request_shape(
            request_id,
            shared,
            provider,
            endpoint,
            false,
            "unchanged",
        );
        return Ok(body);
    }
    let shaped = match prodex_provider_core::shape_harness_request(
        shared.resolved_harness.effective,
        endpoint,
        &body,
        &request.headers,
    ) {
        Ok(shaped) => shaped,
        Err(error) => {
            runtime_harness_log_request_rejection(
                request_id,
                shared,
                provider,
                endpoint,
                error.code(),
            );
            return Err(runtime_local_rewrite_json_parts(
                400,
                json!({
                    "error": {
                        "message": "request is incompatible with the selected minimal harness",
                        "type": "invalid_request_error",
                        "code": "invalid_request",
                    }
                }),
            ));
        }
    };
    let instruction_applied = shaped.applied;
    let body = shaped.body.into_owned();
    let model = runtime_provider_model_from_body(&body).or_else(|| {
        (provider == ProviderId::Gemini)
            .then(|| prodex_provider_core::PRODEX_GEMINI_DEFAULT_MODEL.to_string())
    });
    match prodex_provider_core::shape_harness_provider_request(
        shared.resolved_harness.effective,
        provider,
        model.as_deref(),
        endpoint,
        &body,
    ) {
        Ok(shaped) => {
            runtime_harness_log_provider_policy(
                &shared.runtime_shared,
                request_id,
                RuntimeHarnessProviderPolicyLog {
                    provider,
                    endpoint,
                    model: model.as_deref().unwrap_or_default(),
                    phase: "request",
                    policy: shaped.policy,
                    applied: shaped.applied,
                },
            );
            runtime_harness_log_request_shape(
                request_id,
                shared,
                provider,
                endpoint,
                instruction_applied || shaped.applied,
                "accepted",
            );
            Ok(shaped.body.into_owned())
        }
        Err(error) => {
            runtime_harness_log_request_rejection(
                request_id,
                shared,
                provider,
                endpoint,
                error.code(),
            );
            Err(runtime_local_rewrite_json_parts(
                400,
                json!({
                    "error": {
                        "message": "request is incompatible with the selected evaluated harness",
                        "type": "invalid_request_error",
                        "code": "invalid_request",
                    }
                }),
            ))
        }
    }
}

fn runtime_harness_log_request_rejection(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    reason: &'static str,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "harness_request_shape",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", provider.label()),
                runtime_proxy_log_field("route", endpoint.label()),
                runtime_proxy_log_field("requested", shared.resolved_harness.requested.to_string()),
                runtime_proxy_log_field("resolved", shared.resolved_harness.effective.to_string()),
                runtime_proxy_log_field("applied", "false"),
                runtime_proxy_log_field("outcome", "rejected"),
                runtime_proxy_log_field("reason", reason),
            ],
        ),
    );
}

fn runtime_harness_log_request_shape(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    provider: ProviderId,
    endpoint: ProviderEndpoint,
    applied: bool,
    outcome: &'static str,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "harness_request_shape",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("provider", provider.label()),
                runtime_proxy_log_field("route", endpoint.label()),
                runtime_proxy_log_field("requested", shared.resolved_harness.requested.to_string()),
                runtime_proxy_log_field("resolved", shared.resolved_harness.effective.to_string()),
                runtime_proxy_log_field("applied", applied.to_string()),
                runtime_proxy_log_field("outcome", outcome),
            ],
        ),
    );
}

pub(super) fn runtime_local_rewrite_json_parts(
    status: u16,
    body: Value,
) -> RuntimeHeapTrimmedBufferedResponseParts {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    }
}

pub(super) fn runtime_local_rewrite_route_kind(endpoint: ProviderEndpoint) -> RuntimeRouteKind {
    match endpoint {
        ProviderEndpoint::Responses | ProviderEndpoint::ChatCompletions => {
            RuntimeRouteKind::Responses
        }
        ProviderEndpoint::ResponsesCompact => RuntimeRouteKind::Compact,
        _ => RuntimeRouteKind::Standard,
    }
}

#[cfg(test)]
mod tests {
    use prodex_provider_core::{
        ProviderEndpoint, ProviderId, ProviderTransformInput, ProviderTransformLoss,
        provider_core_lossless_body, provider_translator,
    };

    #[test]
    fn anthropic_provider_core_request_stays_lossless_for_simple_responses_history() {
        let result = provider_translator(ProviderId::Anthropic).transform_request(
            ProviderTransformInput::new(
                ProviderEndpoint::Responses,
                serde_json::to_vec(&serde_json::json!({
                    "model": "claude-sonnet-4-6",
                    "stream": true,
                    "input": [{
                        "type": "message",
                        "role": "user",
                        "content": [{"type": "input_text", "text": "hello"}]
                    }]
                }))
                .unwrap(),
            ),
        );
        assert!(matches!(result.loss, ProviderTransformLoss::Lossless));
        assert!(provider_core_lossless_body(Some(&result)).is_some());
    }
}
