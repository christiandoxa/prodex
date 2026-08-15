use super::anthropic_rewrite::{RuntimeAnthropicAuth, RuntimeAnthropicProviderAuth};
use super::chat_compatible_rewrite::{
    RuntimeDeepSeekRewriteOptions, runtime_provider_chat_compatible_request_body,
};
use super::deepseek_rewrite::{RuntimeDeepSeekConversationStore, RuntimeDeepSeekPendingRequest};
use super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteProxyShared,
    RuntimeLocalRewriteUpstreamResponse, RuntimeLocalRewriteUpstreamResult,
};
use super::local_rewrite_application_data_plane::runtime_gateway_application_provider_retry_precommit;
use super::local_rewrite_model_memory::runtime_local_rewrite_model_selection;
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, RuntimeLocalRewriteSelectedAnthropicAuth,
    runtime_anthropic_messages_upstream_url, runtime_local_rewrite_anthropic_auth_attempts,
    runtime_local_rewrite_upstream_url, runtime_openai_standard_provider_upstream_url,
    send_runtime_local_rewrite_prepared_request,
};
use super::local_rewrite_upstream::{
    RuntimeLocalRewriteBindingContext, RuntimeLocalRewriteLiveResponse,
    RuntimeLocalRewriteNativeFirstEvent, runtime_local_rewrite_attach_accepted_binding,
    runtime_local_rewrite_binding_context, runtime_local_rewrite_precommit_native_first_event,
    runtime_local_rewrite_raw_binding_identity,
};
use super::provider_bridge::{
    RuntimeHarnessProviderPolicyLog, RuntimeProviderBridgeKind, RuntimeProviderErrorClass,
    runtime_harness_log_provider_policy, runtime_provider_error_class, runtime_provider_label,
    runtime_provider_log_request_conformance, runtime_provider_model_fallback_chain,
    runtime_provider_request_body_with_model, runtime_provider_request_conformance_result,
};
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, runtime_proxy_log};
use anyhow::Result;
use prodex_provider_core::{
    ProviderEndpoint, ProviderId, ProviderTransformInput, harness_provider_policy,
    provider_core_lossless_body, translate_openai_chat_request_to_anthropic_messages,
};
use prodex_provider_spi::ProviderRetryCause;
use runtime_anthropic_crate::runtime_anthropic_first_event_retry_allowed;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use serde_json::json;

struct AnthropicDispatchPlan {
    auth_attempts: Vec<RuntimeLocalRewriteSelectedAnthropicAuth>,
    binding: RuntimeLocalRewriteBindingContext,
    route: AnthropicDispatchRoute,
}

struct AnthropicPassthroughPlan {
    upstream_url: String,
    body: Vec<u8>,
    native_messages: bool,
}

enum AnthropicDispatchRoute {
    Responses(AnthropicResponsesPlan),
    Passthrough(AnthropicPassthroughPlan),
}

struct AnthropicResponsesPlan {
    body: Vec<u8>,
    model_chain: Vec<String>,
    chat_upstream_url: String,
    messages_upstream_url: String,
}

struct AnthropicSendContext<'a> {
    request_id: u64,
    request: &'a RuntimeProxyRequest,
    shared: &'a RuntimeLocalRewriteProxyShared,
    binding: &'a RuntimeLocalRewriteBindingContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AnthropicAttempt {
    auth_index: usize,
    model_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AnthropicAttemptCursor {
    auth_count: usize,
    model_count: usize,
    current: Option<AnthropicAttempt>,
}

impl AnthropicAttemptCursor {
    fn new(auth_count: usize, model_count: usize) -> Self {
        Self {
            auth_count,
            model_count,
            current: (auth_count > 0 && model_count > 0).then_some(AnthropicAttempt {
                auth_index: 0,
                model_index: 0,
            }),
        }
    }

    fn current(&self) -> Option<AnthropicAttempt> {
        self.current
    }

    fn next_model(&mut self) {
        let Some(mut attempt) = self.current else {
            return;
        };
        attempt.model_index += 1;
        self.current = (attempt.model_index < self.model_count).then_some(attempt);
    }

    fn next_credential(&mut self) {
        let Some(mut attempt) = self.current else {
            return;
        };
        attempt.auth_index += 1;
        attempt.model_index = 0;
        self.current = (attempt.auth_index < self.auth_count).then_some(attempt);
    }
}

enum AnthropicAttemptOutcome {
    ModelFallback {
        status: u16,
        class: RuntimeProviderErrorClass,
    },
    CredentialRotation {
        status: u16,
        class: RuntimeProviderErrorClass,
    },
    TerminalBuffered(RuntimeHeapTrimmedBufferedResponseParts),
    SuccessfulLive {
        response: RuntimeLocalRewriteLiveResponse,
        pending_request: RuntimeDeepSeekPendingRequest,
    },
    NativeFirstEvent {
        response: RuntimeLocalRewriteLiveResponse,
        class: RuntimeProviderErrorClass,
        pending_request: RuntimeDeepSeekPendingRequest,
    },
    InternalFailure(anyhow::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnthropicRetryTransition {
    ModelFallback,
    CredentialRotation,
    TerminalBuffered,
}

pub(super) fn send_runtime_anthropic_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeAnthropicProviderAuth,
    endpoint: ProviderEndpoint,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let plan = AnthropicDispatchPlan::new(request, shared, body, auth, endpoint)?;
    let AnthropicDispatchPlan {
        auth_attempts,
        binding,
        route,
    } = plan;
    match route {
        AnthropicDispatchRoute::Responses(responses) => send_responses_attempts(
            request_id,
            request,
            shared,
            auth_attempts,
            &binding,
            responses,
        ),
        AnthropicDispatchRoute::Passthrough(plan) => {
            send_passthrough_attempts(request_id, request, shared, auth_attempts, &binding, plan)
        }
    }
}

impl AnthropicDispatchPlan {
    fn new(
        request: &RuntimeProxyRequest,
        shared: &RuntimeLocalRewriteProxyShared,
        body: Vec<u8>,
        auth: &RuntimeAnthropicProviderAuth,
        endpoint: ProviderEndpoint,
    ) -> Result<Self> {
        let binding = runtime_local_rewrite_binding_context(shared, request)?;
        let mut auth_attempts = runtime_local_rewrite_anthropic_auth_attempts(shared, auth);
        auth_attempts.retain(|selected| {
            runtime_anthropic_binding_identity(selected, shared)
                .as_ref()
                .is_some_and(|identity| binding.candidate_allowed(Some(identity)))
        });
        if auth_attempts.is_empty() {
            if binding.bound.is_some() {
                anyhow::bail!("Anthropic continuation binding is unavailable or unauthorized");
            }
            anyhow::bail!("Anthropic provider has no auth configured");
        }
        let route = if endpoint == ProviderEndpoint::Responses {
            let model_selection = runtime_local_rewrite_model_selection(
                shared,
                RuntimeProviderBridgeKind::Anthropic,
                request,
                &body,
                prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
            );
            AnthropicDispatchRoute::Responses(AnthropicResponsesPlan {
                model_chain: runtime_provider_model_fallback_chain(
                    RuntimeProviderBridgeKind::Anthropic,
                    &model_selection.model,
                ),
                body: model_selection.body,
                chat_upstream_url: runtime_openai_standard_provider_upstream_url(
                    RuntimeProviderBridgeKind::Anthropic,
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                ),
                messages_upstream_url: runtime_anthropic_messages_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                ),
            })
        } else {
            AnthropicDispatchRoute::Passthrough(AnthropicPassthroughPlan {
                upstream_url: runtime_local_rewrite_upstream_url(
                    &shared.upstream_base_url,
                    &shared.mount_path,
                    &request.path_and_query,
                ),
                body,
                native_messages: endpoint == ProviderEndpoint::Messages,
            })
        };
        Ok(Self {
            auth_attempts,
            binding,
            route,
        })
    }
}

fn runtime_anthropic_binding_identity(
    selected: &RuntimeLocalRewriteSelectedAnthropicAuth,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Option<prodex_provider_core::RuntimeProviderBindingIdentity> {
    let (credential, profile) = match &selected.auth {
        RuntimeAnthropicAuth::ApiKey { api_key } => (Some(api_key.as_str()), None),
        RuntimeAnthropicAuth::OAuth { access_token } => {
            (Some(access_token.as_str()), Some(selected.label.as_str()))
        }
        RuntimeAnthropicAuth::Projected => (None, Some(RUNTIME_LOCAL_REWRITE_PROFILE)),
    };
    runtime_local_rewrite_raw_binding_identity(
        shared,
        ProviderId::Anthropic,
        credential,
        &shared.upstream_base_url,
        profile,
    )
}

fn send_responses_attempts(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    auth_attempts: Vec<RuntimeLocalRewriteSelectedAnthropicAuth>,
    binding: &RuntimeLocalRewriteBindingContext,
    plan: AnthropicResponsesPlan,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let AnthropicResponsesPlan {
        body,
        model_chain,
        chat_upstream_url,
        messages_upstream_url,
    } = plan;
    let auth_count = auth_attempts.len();
    let mut cursor = AnthropicAttemptCursor::new(auth_count, model_chain.len());
    let conversations = shared.deepseek_conversations_for_request(request);
    let context = AnthropicSendContext {
        request_id,
        request,
        shared,
        binding,
    };
    let mut first_event_retries = 0;
    while let Some(attempt) = cursor.current() {
        let selected_auth = &auth_attempts[attempt.auth_index];
        let model = &model_chain[attempt.model_index];
        let Some(prepared) = prepare_anthropic_attempt(
            &context,
            &body,
            &conversations,
            model,
            &chat_upstream_url,
            &messages_upstream_url,
        )?
        else {
            return Ok(incompatible_native_translation());
        };
        let outcome =
            send_anthropic_attempt(&context, selected_auth, model, attempt, &cursor, prepared);
        match outcome {
            AnthropicAttemptOutcome::ModelFallback { status, class } => {
                log_model_fallback(
                    request_id,
                    shared,
                    selected_auth.label.as_str(),
                    model,
                    &model_chain[attempt.model_index + 1],
                    status,
                    class,
                );
                cursor.next_model();
            }
            AnthropicAttemptOutcome::CredentialRotation { status, class } => {
                log_auth_rotation(
                    request_id,
                    shared,
                    selected_auth.label.as_str(),
                    status,
                    class,
                );
                cursor.next_credential();
            }
            AnthropicAttemptOutcome::TerminalBuffered(parts) => return Ok(buffered(parts)),
            AnthropicAttemptOutcome::SuccessfulLive {
                response,
                pending_request,
            } => return Ok(live_response(response, pending_request)),
            AnthropicAttemptOutcome::NativeFirstEvent {
                response,
                class,
                pending_request,
            } => {
                let can_retry =
                    runtime_anthropic_first_event_retry_allowed(first_event_retries, false);
                if can_retry
                    && attempt.model_index + 1 < model_chain.len()
                    && runtime_gateway_application_provider_retry_precommit(
                        ProviderRetryCause::NextModel,
                        class,
                        attempt.model_index,
                        model_chain.len(),
                    )
                {
                    log_model_fallback(
                        request_id,
                        shared,
                        selected_auth.label.as_str(),
                        model,
                        &model_chain[attempt.model_index + 1],
                        200,
                        class,
                    );
                    first_event_retries += 1;
                    cursor.next_model();
                    continue;
                }
                if can_retry
                    && runtime_gateway_application_provider_retry_precommit(
                        ProviderRetryCause::RotateCredential,
                        class,
                        attempt.auth_index,
                        auth_count,
                    )
                {
                    log_auth_rotation(request_id, shared, selected_auth.label.as_str(), 200, class);
                    first_event_retries += 1;
                    cursor.next_credential();
                    continue;
                }
                return Ok(live_response(response, pending_request));
            }
            AnthropicAttemptOutcome::InternalFailure(error) => return Err(error),
        }
    }
    anyhow::bail!("no Anthropic model attempts were available")
}

struct AnthropicPreparedAttempt<'a> {
    upstream_url: &'a str,
    upstream_body: Vec<u8>,
    native_messages: bool,
    pending_request: RuntimeDeepSeekPendingRequest,
}

fn prepare_anthropic_attempt<'a>(
    context: &AnthropicSendContext<'_>,
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
    model: &str,
    chat_upstream_url: &'a str,
    messages_upstream_url: &'a str,
) -> Result<Option<AnthropicPreparedAttempt<'a>>> {
    let model_body = runtime_provider_request_body_with_model(body, model);
    let harness_policy = harness_provider_policy(
        context.shared.resolved_harness.effective,
        ProviderId::Anthropic,
        Some(model),
    );
    let native_messages = harness_policy.is_some_and(|policy| policy.native_anthropic_messages);
    let translated = runtime_provider_chat_compatible_request_body(
        &model_body,
        conversations,
        RuntimeProviderBridgeKind::Anthropic,
        prodex_cli::SUPER_ANTHROPIC_DEFAULT_MODEL,
        false,
        RuntimeDeepSeekRewriteOptions::default(),
    )?;
    let provider_core_result = if native_messages {
        let mut input =
            ProviderTransformInput::new(ProviderEndpoint::Responses, translated.body.clone());
        input.model = Some(model.to_string());
        Some(translate_openai_chat_request_to_anthropic_messages(input))
    } else {
        runtime_provider_request_conformance_result(
            RuntimeProviderBridgeKind::Anthropic,
            context.request,
            &model_body,
        )
    };
    if let Some(result) = provider_core_result.as_ref() {
        runtime_provider_log_request_conformance(
            &context.shared.runtime_shared,
            context.request_id,
            RuntimeProviderBridgeKind::Anthropic,
            result,
        );
    }
    runtime_harness_log_provider_policy(
        &context.shared.runtime_shared,
        context.request_id,
        RuntimeHarnessProviderPolicyLog {
            provider: ProviderId::Anthropic,
            endpoint: ProviderEndpoint::Responses,
            model,
            phase: "request-translation",
            policy: harness_policy,
            applied: native_messages
                && provider_core_lossless_body(provider_core_result.as_ref()).is_some(),
        },
    );
    let Some(upstream_body) = provider_core_lossless_body(provider_core_result.as_ref())
        .or_else(|| (!native_messages).then(|| translated.body.clone()))
    else {
        return Ok(None);
    };
    Ok(Some(AnthropicPreparedAttempt {
        upstream_url: if native_messages {
            messages_upstream_url
        } else {
            chat_upstream_url
        },
        upstream_body,
        native_messages,
        pending_request: RuntimeDeepSeekPendingRequest {
            messages: translated.messages,
            response_metadata: translated.response_metadata,
        },
    }))
}

fn send_anthropic_attempt(
    context: &AnthropicSendContext<'_>,
    selected_auth: &RuntimeLocalRewriteSelectedAnthropicAuth,
    model: &str,
    attempt: AnthropicAttempt,
    cursor: &AnthropicAttemptCursor,
    prepared: AnthropicPreparedAttempt<'_>,
) -> AnthropicAttemptOutcome {
    let AnthropicPreparedAttempt {
        upstream_url,
        upstream_body,
        native_messages,
        pending_request,
    } = prepared;
    let send_result = send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
        RuntimeLocalRewriteSearchFallbackRequest {
            request_id: context.request_id,
            request: context.request,
            shared: context.shared,
            upstream_url,
            body: upstream_body,
            provider_kind: RuntimeProviderBridgeKind::Anthropic,
            auth_label: selected_auth.label.as_str(),
            model,
            auth_factory: || RuntimeLocalRewritePreparedAuth::Anthropic {
                auth: &selected_auth.auth,
                native_messages,
            },
        },
    );
    match send_result {
        Ok(RuntimeLocalRewritePreparedSendResult::Live(response)) => {
            let Some(binding_identity) =
                runtime_anthropic_binding_identity(selected_auth, context.shared)
            else {
                return AnthropicAttemptOutcome::InternalFailure(anyhow::anyhow!(
                    "Anthropic accepted binding identity is unavailable"
                ));
            };
            let mut live_response = if native_messages {
                RuntimeLocalRewriteLiveResponse::with_native_anthropic_messages(response)
            } else {
                RuntimeLocalRewriteLiveResponse::new(response)
            };
            runtime_local_rewrite_attach_accepted_binding(
                context.shared,
                &mut live_response,
                context.binding,
                binding_identity,
            );
            if native_messages {
                match runtime_local_rewrite_precommit_native_first_event(
                    &mut live_response,
                    RuntimeProviderBridgeKind::Anthropic,
                    context
                        .shared
                        .runtime_shared
                        .runtime_config
                        .tuning
                        .sse_lookahead_timeout_ms,
                    &context.shared.provider_sse_prefetch_slots,
                ) {
                    Ok(RuntimeLocalRewriteNativeFirstEvent::Retry(class)) => {
                        return AnthropicAttemptOutcome::NativeFirstEvent {
                            response: live_response,
                            class,
                            pending_request,
                        };
                    }
                    Ok(RuntimeLocalRewriteNativeFirstEvent::Commit) => {}
                    Err(error) => return AnthropicAttemptOutcome::InternalFailure(error),
                }
            }
            AnthropicAttemptOutcome::SuccessfulLive {
                response: live_response,
                pending_request,
            }
        }
        Ok(RuntimeLocalRewritePreparedSendResult::Error {
            status,
            parts,
            class,
        }) => classify_buffered_outcome(attempt, cursor, status, parts, class),
        Err(error) => AnthropicAttemptOutcome::InternalFailure(error),
    }
}

fn classify_buffered_outcome(
    attempt: AnthropicAttempt,
    cursor: &AnthropicAttemptCursor,
    status: u16,
    parts: RuntimeHeapTrimmedBufferedResponseParts,
    class: RuntimeProviderErrorClass,
) -> AnthropicAttemptOutcome {
    match classify_retry_transition(attempt, cursor, class) {
        AnthropicRetryTransition::ModelFallback => {
            AnthropicAttemptOutcome::ModelFallback { status, class }
        }
        AnthropicRetryTransition::CredentialRotation => {
            AnthropicAttemptOutcome::CredentialRotation { status, class }
        }
        AnthropicRetryTransition::TerminalBuffered => {
            AnthropicAttemptOutcome::TerminalBuffered(parts)
        }
    }
}

fn classify_retry_transition(
    attempt: AnthropicAttempt,
    cursor: &AnthropicAttemptCursor,
    class: RuntimeProviderErrorClass,
) -> AnthropicRetryTransition {
    if attempt.model_index + 1 < cursor.model_count
        && runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::NextModel,
            class,
            attempt.model_index,
            cursor.model_count,
        )
    {
        return AnthropicRetryTransition::ModelFallback;
    }
    if runtime_gateway_application_provider_retry_precommit(
        ProviderRetryCause::RotateCredential,
        class,
        attempt.auth_index,
        cursor.auth_count,
    ) {
        return AnthropicRetryTransition::CredentialRotation;
    }
    AnthropicRetryTransition::TerminalBuffered
}

fn send_passthrough_attempts(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    auth_attempts: Vec<RuntimeLocalRewriteSelectedAnthropicAuth>,
    binding: &RuntimeLocalRewriteBindingContext,
    plan: AnthropicPassthroughPlan,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let AnthropicPassthroughPlan {
        upstream_url,
        body,
        native_messages,
    } = plan;
    let auth_count = auth_attempts.len();
    for (auth_index, selected_auth) in auth_attempts.iter().enumerate() {
        let response = send_runtime_local_rewrite_prepared_request(
            request_id,
            request,
            shared,
            &upstream_url,
            body.clone(),
            RuntimeLocalRewritePreparedAuth::Anthropic {
                auth: &selected_auth.auth,
                native_messages,
            },
        )?;
        let status = response.status().as_u16();
        if status < 400 {
            let Some(binding_identity) = runtime_anthropic_binding_identity(selected_auth, shared)
            else {
                anyhow::bail!("Anthropic accepted binding identity is unavailable");
            };
            let mut accepted_response = RuntimeLocalRewriteLiveResponse::new(response);
            runtime_local_rewrite_attach_accepted_binding(
                shared,
                &mut accepted_response,
                binding,
                binding_identity,
            );
            return Ok(live_response(
                accepted_response,
                RuntimeDeepSeekPendingRequest::default(),
            ));
        }
        let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
        let class =
            runtime_provider_error_class(RuntimeProviderBridgeKind::Anthropic, status, &parts.body);
        if runtime_gateway_application_provider_retry_precommit(
            ProviderRetryCause::RotateCredential,
            class,
            auth_index,
            auth_count,
        ) {
            log_auth_rotation(
                request_id,
                shared,
                selected_auth.label.as_str(),
                status,
                class,
            );
            continue;
        }
        return Ok(buffered(parts));
    }
    anyhow::bail!("no Anthropic auth attempts were available")
}

fn incompatible_native_translation() -> RuntimeLocalRewriteUpstreamResult {
    buffered(
        super::local_rewrite_upstream::runtime_local_rewrite_json_parts(
            400,
            json!({
                "error": {
                    "message": "request is incompatible with evaluated Anthropic Messages translation",
                    "type": "invalid_request_error",
                    "code": "invalid_request",
                }
            }),
        ),
    )
}

fn buffered(parts: RuntimeHeapTrimmedBufferedResponseParts) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
        gemini_context: None,
        copilot_context: None,
    }
}

fn live_response(
    response: RuntimeLocalRewriteLiveResponse,
    pending_request: RuntimeDeepSeekPendingRequest,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Live(
            response.with_chat_compatible_request(pending_request),
        ),
        gemini_context: None,
        copilot_context: None,
    }
}

fn log_model_fallback(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    auth_label: &str,
    from_model: &str,
    to_model: &str,
    status: u16,
    class: RuntimeProviderErrorClass,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_model_fallback",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::Anthropic),
                ),
                runtime_proxy_log_field("auth", auth_label),
                runtime_proxy_log_field("from_model", from_model),
                runtime_proxy_log_field("to_model", to_model),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("class", format!("{class:?}")),
            ],
        ),
    );
}

fn log_auth_rotation(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    auth_label: &str,
    status: u16,
    class: RuntimeProviderErrorClass,
) {
    runtime_proxy_log(
        &shared.runtime_shared,
        runtime_proxy_structured_log_message(
            "local_rewrite_provider_auth_rotate",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field(
                    "provider",
                    runtime_provider_label(RuntimeProviderBridgeKind::Anthropic),
                ),
                runtime_proxy_log_field("auth", auth_label),
                runtime_proxy_log_field("status", status.to_string()),
                runtime_proxy_log_field("class", format!("{class:?}")),
            ],
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        AnthropicAttempt, AnthropicAttemptCursor, AnthropicRetryTransition,
        classify_retry_transition,
    };
    use prodex_provider_core::ProviderErrorClass;

    #[test]
    fn anthropic_attempt_cursor_exhausts_models_before_rotating_credentials() {
        let mut cursor = AnthropicAttemptCursor::new(2, 3);
        let mut attempts = Vec::new();
        while let Some(attempt) = cursor.current() {
            attempts.push(attempt);
            if attempt.model_index + 1 < cursor.model_count {
                cursor.next_model();
            } else {
                cursor.next_credential();
            }
        }
        assert_eq!(
            attempts,
            vec![
                AnthropicAttempt {
                    auth_index: 0,
                    model_index: 0,
                },
                AnthropicAttempt {
                    auth_index: 0,
                    model_index: 1,
                },
                AnthropicAttempt {
                    auth_index: 0,
                    model_index: 2,
                },
                AnthropicAttempt {
                    auth_index: 1,
                    model_index: 0,
                },
                AnthropicAttempt {
                    auth_index: 1,
                    model_index: 1,
                },
                AnthropicAttempt {
                    auth_index: 1,
                    model_index: 2,
                },
            ]
        );
    }

    #[test]
    fn anthropic_attempt_cursor_never_creates_empty_attempts() {
        assert_eq!(AnthropicAttemptCursor::new(0, 1).current(), None);
        assert_eq!(AnthropicAttemptCursor::new(1, 0).current(), None);
    }

    #[test]
    fn anthropic_buffered_outcomes_have_explicit_retry_transitions() {
        let cursor = AnthropicAttemptCursor::new(2, 2);
        let first = cursor.current().unwrap();
        assert_eq!(
            classify_retry_transition(first, &cursor, ProviderErrorClass::NotFound),
            AnthropicRetryTransition::ModelFallback
        );

        let last_model = AnthropicAttempt {
            auth_index: 0,
            model_index: 1,
        };
        assert_eq!(
            classify_retry_transition(last_model, &cursor, ProviderErrorClass::Auth),
            AnthropicRetryTransition::CredentialRotation
        );
        assert_eq!(
            classify_retry_transition(last_model, &cursor, ProviderErrorClass::Other),
            AnthropicRetryTransition::TerminalBuffered
        );

        let final_attempt = AnthropicAttempt {
            auth_index: 1,
            model_index: 1,
        };
        assert_eq!(
            classify_retry_transition(final_attempt, &cursor, ProviderErrorClass::Transient),
            AnthropicRetryTransition::TerminalBuffered
        );
    }
}
