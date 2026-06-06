use super::gemini_rewrite::{
    RuntimeGeminiAuth, RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth,
    RuntimeGeminiTranslatedRequest, runtime_gemini_finish_reason,
    runtime_gemini_finish_reason_retryable_invalid, runtime_gemini_generate_request_body,
    runtime_gemini_media_content_item_from_part, runtime_gemini_normalized_response_value,
    runtime_gemini_project_id, runtime_gemini_prompt_feedback_failure,
    runtime_gemini_request_body_without_tool, runtime_gemini_text_from_special_part,
    runtime_gemini_upstream_url,
};
use super::gemini_sse::RuntimeGeminiBindingRecorder;
use super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
pub(super) use super::local_rewrite_gemini_bindings::runtime_gemini_remember_bindings_from_responses_body;
use super::local_rewrite_gemini_bindings::runtime_gemini_tool_output_call_ids_from_request;
use super::local_rewrite_gemini_quota::{
    runtime_gemini_body_has_terminal_quota, runtime_gemini_buffered_parts_are_quota_blocked,
    runtime_gemini_normalized_error_parts, runtime_gemini_response_retryable_quota,
    runtime_gemini_retry_delay_ms,
};
use super::local_rewrite_gemini_thought_signatures::runtime_gemini_harden_translated_thoughts as harden_thoughts;
use super::local_rewrite_rate_limits::runtime_gemini_quota_codex_headers;
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_local_rewrite_api_key_attempts,
    send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_error_cooldown_ms, runtime_provider_model_fallback_chain,
    runtime_provider_model_from_body, runtime_provider_request_body_with_model,
    runtime_provider_should_retry_with_next_model,
};
use crate::{
    RuntimeProxyRequest, fetch_gemini_quota_with_code_assist_endpoint, gemini_code_assist_endpoint,
    runtime_proxy_log, runtime_proxy_log_to_path, spawn_runtime_background_worker_or_log,
};
use anyhow::{Context, Result, bail};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT: usize = 4096;
const RUNTIME_GEMINI_LOCAL_RETRY_LIMIT: usize = 9;
const RUNTIME_GEMINI_INVALID_STREAM_RETRY_LIMIT: usize = 3;
const RUNTIME_GEMINI_INVALID_STREAM_RETRY_BASE_DELAY_MS: u64 = 1_000;
const RUNTIME_GEMINI_PRECOMMIT_PEEK_LIMIT: usize = 64 * 1024;

#[path = "local_rewrite_gemini_auth.rs"]
mod local_rewrite_gemini_auth;

#[derive(Clone)]
pub(super) struct RuntimeGeminiOAuthPool {
    state: Arc<Mutex<RuntimeGeminiOAuthPoolState>>,
}

#[derive(Debug)]
struct RuntimeGeminiOAuthPoolState {
    profiles: Vec<RuntimeGeminiOAuthProfileAuth>,
    next_index: usize,
    response_profile_bindings: BTreeMap<String, String>,
    tool_call_profile_bindings: BTreeMap<String, String>,
    quota_headers: BTreeMap<String, Vec<(String, String)>>,
    model_cooldowns_until: BTreeMap<String, u64>,
}

#[derive(Clone)]
struct RuntimeGeminiSelectedAuth {
    profile_name: String,
    auth: RuntimeGeminiAuth,
    hard_affinity: bool,
}

#[derive(Clone)]
pub(super) struct RuntimeGeminiRequestContext {
    pub(super) profile_name: String,
    pub(super) conversation_messages: Vec<serde_json::Value>,
    pub(super) binding_recorder: Option<RuntimeGeminiBindingRecorder>,
}

pub(super) fn runtime_gemini_oauth_pool_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<RuntimeGeminiOAuthPool> {
    let RuntimeLocalRewriteProviderOptions::Gemini {
        auth: RuntimeGeminiProviderAuth::OAuthProfiles { profiles },
        ..
    } = provider
    else {
        return None;
    };
    Some(RuntimeGeminiOAuthPool {
        state: Arc::new(Mutex::new(RuntimeGeminiOAuthPoolState {
            profiles: profiles.clone(),
            next_index: 0,
            response_profile_bindings: BTreeMap::new(),
            tool_call_profile_bindings: BTreeMap::new(),
            quota_headers: BTreeMap::new(),
            model_cooldowns_until: BTreeMap::new(),
        })),
    })
}

pub(super) fn send_runtime_gemini_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeGeminiProviderAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let responses_route = path_without_query(&request.path_and_query).ends_with("/responses");
    let thinking_budget_tokens = runtime_gemini_thinking_budget_tokens(&shared.provider);
    let attempts = runtime_gemini_auth_attempts(auth, shared, &body)?;
    let attempt_count = attempts.len();
    'auth_attempts: for (attempt_index, mut selected) in attempts.into_iter().enumerate() {
        let requested_model = runtime_provider_model_from_body(&body)
            .unwrap_or_else(|| prodex_cli::SUPER_GEMINI_DEFAULT_MODEL.to_string());
        let mut model_chain = if responses_route {
            runtime_provider_model_fallback_chain(
                RuntimeProviderBridgeKind::Gemini,
                &requested_model,
            )
        } else {
            vec![prodex_cli::SUPER_GEMINI_DEFAULT_MODEL.to_string()]
        };
        // Gemini Code Assist (OAuth) does not serve customtools models; filter them from the fallback chain.
        if matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }) {
            model_chain.retain(|m| !m.contains("customtools"));
            if model_chain.is_empty() {
                model_chain.push(prodex_cli::SUPER_GEMINI_DEFAULT_MODEL.to_string());
            }
        }
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = if responses_route {
                runtime_provider_request_body_with_model(&body, model)
            } else {
                body.clone()
            };
            let mut translated = if responses_route {
                runtime_gemini_generate_request_body(
                    &model_body,
                    &shared.gemini_conversations,
                    matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }),
                    runtime_gemini_project_id(&selected.auth),
                    thinking_budget_tokens,
                )?
            } else {
                RuntimeGeminiTranslatedRequest {
                    body: body.clone(),
                    messages: Vec::new(),
                    model: model.clone(),
                    stream: false,
                }
            };
            harden_thoughts(
                shared,
                request_id,
                selected.profile_name.as_str(),
                &mut translated,
            )?;
            let upstream_url = runtime_gemini_upstream_url(
                &shared.upstream_base_url,
                &selected.auth,
                &translated.model,
                translated.stream,
            );
            let mut rate_limit_retry_index = 0;
            let mut invalid_stream_retry_index = 0;
            let mut auth_refresh_attempted = false;
            loop {
                let mut response = send_runtime_local_rewrite_prepared_request(
                    request_id,
                    request,
                    shared,
                    &upstream_url,
                    translated.body.clone(),
                    RuntimeLocalRewritePreparedAuth::Gemini {
                        auth: &selected.auth,
                    },
                )?;
                let status = response.status().as_u16();
                if status >= 400 {
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_string);
                    let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
                    let class = runtime_provider_error_class(
                        RuntimeProviderBridgeKind::Gemini,
                        status,
                        &parts.body,
                    );
                    let quota_blocked =
                        runtime_gemini_buffered_parts_are_quota_blocked(status, &parts);
                    let delay_ms = runtime_gemini_retry_delay_ms(
                        retry_after.as_deref(),
                        &parts.body,
                        rate_limit_retry_index,
                    )
                    .unwrap_or_else(|| {
                        runtime_provider_error_cooldown_ms(class, status, &parts.body)
                    });
                    if matches!(
                        class,
                        RuntimeProviderErrorClass::Quota
                            | RuntimeProviderErrorClass::RateLimit
                            | RuntimeProviderErrorClass::Transient
                    ) && let Some(pool) = shared.gemini_oauth_pool.as_ref()
                    {
                        pool.remember_model_cooldown(
                            &selected.profile_name,
                            &translated.model,
                            delay_ms,
                        );
                    }
                    if status == 400
                        && let Some((tool_name, fallback_body)) =
                            runtime_gemini_unsupported_tool_fallback_body(&translated.body)
                    {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_gemini_builtin_tool_fallback",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field("model", translated.model.as_str()),
                                    runtime_proxy_log_field("status", status.to_string()),
                                    runtime_proxy_log_field("tool", tool_name),
                                ],
                            ),
                        );
                        translated.body = fallback_body;
                        continue;
                    }
                    if runtime_provider_should_retry_with_next_model(class)
                        && model_index + 1 < model_chain.len()
                    {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_provider_model_fallback",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field("provider", "gemini"),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field(
                                        "from_model",
                                        translated.model.as_str(),
                                    ),
                                    runtime_proxy_log_field(
                                        "to_model",
                                        model_chain[model_index + 1].as_str(),
                                    ),
                                    runtime_proxy_log_field("status", status.to_string()),
                                    runtime_proxy_log_field("class", format!("{class:?}")),
                                ],
                            ),
                        );
                        break;
                    }
                    if class == RuntimeProviderErrorClass::Auth {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_provider_auth_failure",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field("provider", "gemini"),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field("status", status.to_string()),
                                ],
                            ),
                        );
                        if !auth_refresh_attempted
                            && let Some(pool) = shared.gemini_oauth_pool.as_ref()
                        {
                            auth_refresh_attempted = true;
                            match pool.refresh_profile_auth(
                                &selected.profile_name,
                                selected.hard_affinity,
                            ) {
                                Ok(Some(refreshed)) => {
                                    selected = refreshed;
                                    runtime_proxy_log(
                                        &shared.runtime_shared,
                                        runtime_proxy_structured_log_message(
                                            "local_rewrite_provider_auth_refresh",
                                            [
                                                runtime_proxy_log_field(
                                                    "request",
                                                    request_id.to_string(),
                                                ),
                                                runtime_proxy_log_field("provider", "gemini"),
                                                runtime_proxy_log_field(
                                                    "profile",
                                                    selected.profile_name.as_str(),
                                                ),
                                                runtime_proxy_log_field(
                                                    "status",
                                                    status.to_string(),
                                                ),
                                            ],
                                        ),
                                    );
                                    continue;
                                }
                                Ok(None) => {}
                                Err(err) => {
                                    runtime_proxy_log(
                                        &shared.runtime_shared,
                                        runtime_proxy_structured_log_message(
                                            "local_rewrite_provider_auth_refresh_failed",
                                            [
                                                runtime_proxy_log_field(
                                                    "request",
                                                    request_id.to_string(),
                                                ),
                                                runtime_proxy_log_field("provider", "gemini"),
                                                runtime_proxy_log_field(
                                                    "profile",
                                                    selected.profile_name.as_str(),
                                                ),
                                                runtime_proxy_log_field("error", err.to_string()),
                                            ],
                                        ),
                                    );
                                }
                            }
                        }
                        if !selected.hard_affinity && attempt_index + 1 < attempt_count {
                            continue 'auth_attempts;
                        }
                    }
                    if runtime_gemini_should_rotate_after_quota_response(
                        status,
                        selected.hard_affinity,
                        attempt_index,
                        attempt_count,
                    ) && (status == 429 || quota_blocked)
                    {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_gemini_quota_rotate",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field("status", status.to_string()),
                                    runtime_proxy_log_field(
                                        "reason",
                                        if status == 429 {
                                            "rate_limit"
                                        } else {
                                            "quota_body"
                                        },
                                    ),
                                ],
                            ),
                        );
                        continue 'auth_attempts;
                    }

                    if status == 429
                        && !runtime_gemini_body_has_terminal_quota(&parts.body)
                        && delay_ms > 0
                        && rate_limit_retry_index < RUNTIME_GEMINI_LOCAL_RETRY_LIMIT
                    {
                        runtime_proxy_log(
                            &shared.runtime_shared,
                            runtime_proxy_structured_log_message(
                                "local_rewrite_gemini_rate_limit_retry",
                                [
                                    runtime_proxy_log_field("request", request_id.to_string()),
                                    runtime_proxy_log_field(
                                        "profile",
                                        selected.profile_name.as_str(),
                                    ),
                                    runtime_proxy_log_field("status", status.to_string()),
                                    runtime_proxy_log_field(
                                        "retry",
                                        rate_limit_retry_index.to_string(),
                                    ),
                                    runtime_proxy_log_field("delay_ms", delay_ms.to_string()),
                                ],
                            ),
                        );
                        rate_limit_retry_index += 1;
                        thread::sleep(Duration::from_millis(delay_ms));
                        continue;
                    }

                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Buffered(
                            runtime_gemini_normalized_error_parts(status, parts),
                        ),
                        gemini_context: None,
                        copilot_context: None,
                    });
                }

                let mut stream_prefix = Vec::new();
                if responses_route && translated.stream && runtime_gemini_response_is_sse(&response)
                {
                    match runtime_gemini_peek_stream_for_retry(response)? {
                        RuntimeGeminiPrecommitPeek::Committed {
                            response: next_response,
                            prefix,
                        } => {
                            response = next_response;
                            stream_prefix = prefix;
                        }
                        RuntimeGeminiPrecommitPeek::RetryableInvalid {
                            response: next_response,
                            prefix,
                            reason,
                        } => {
                            if invalid_stream_retry_index
                                < RUNTIME_GEMINI_INVALID_STREAM_RETRY_LIMIT
                            {
                                let delay_ms = runtime_gemini_invalid_stream_retry_delay_ms(
                                    invalid_stream_retry_index,
                                );
                                runtime_proxy_log(
                                    &shared.runtime_shared,
                                    runtime_proxy_structured_log_message(
                                        "local_rewrite_gemini_invalid_stream_retry",
                                        [
                                            runtime_proxy_log_field(
                                                "request",
                                                request_id.to_string(),
                                            ),
                                            runtime_proxy_log_field(
                                                "profile",
                                                selected.profile_name.as_str(),
                                            ),
                                            runtime_proxy_log_field(
                                                "model",
                                                translated.model.as_str(),
                                            ),
                                            runtime_proxy_log_field(
                                                "retry",
                                                invalid_stream_retry_index.to_string(),
                                            ),
                                            runtime_proxy_log_field("reason", reason.as_str()),
                                            runtime_proxy_log_field(
                                                "delay_ms",
                                                delay_ms.to_string(),
                                            ),
                                        ],
                                    ),
                                );
                                invalid_stream_retry_index += 1;
                                thread::sleep(Duration::from_millis(delay_ms));
                                continue;
                            }
                            if model_index + 1 < model_chain.len() {
                                runtime_proxy_log(
                                    &shared.runtime_shared,
                                    runtime_proxy_structured_log_message(
                                        "local_rewrite_gemini_invalid_stream_model_fallback",
                                        [
                                            runtime_proxy_log_field(
                                                "request",
                                                request_id.to_string(),
                                            ),
                                            runtime_proxy_log_field(
                                                "profile",
                                                selected.profile_name.as_str(),
                                            ),
                                            runtime_proxy_log_field(
                                                "from_model",
                                                translated.model.as_str(),
                                            ),
                                            runtime_proxy_log_field(
                                                "to_model",
                                                model_chain[model_index + 1].as_str(),
                                            ),
                                            runtime_proxy_log_field("reason", reason.as_str()),
                                        ],
                                    ),
                                );
                                drop(next_response);
                                break;
                            }
                            response = next_response;
                            stream_prefix = prefix;
                        }
                    }
                }

                let binding_recorder = shared.gemini_oauth_pool.as_ref().map(|pool| {
                    runtime_gemini_binding_recorder(pool, selected.profile_name.clone())
                });
                let gemini_context = responses_route.then(|| RuntimeGeminiRequestContext {
                    profile_name: selected.profile_name.clone(),
                    conversation_messages: translated.messages,
                    binding_recorder,
                });
                return Ok(RuntimeLocalRewriteUpstreamResult {
                    response: RuntimeLocalRewriteUpstreamResponse::Live(
                        RuntimeLocalRewriteLiveResponse::with_prefix(response, stream_prefix),
                    ),
                    gemini_context,
                    copilot_context: None,
                });
            }
        }
    }

    bail!("no Gemini auth attempts were available")
}

fn runtime_gemini_thinking_budget_tokens(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<u64> {
    match provider {
        RuntimeLocalRewriteProviderOptions::Gemini {
            thinking_budget_tokens,
            ..
        } => *thinking_budget_tokens,
        _ => None,
    }
}

fn runtime_gemini_unsupported_tool_fallback_body(body: &[u8]) -> Option<(&'static str, Vec<u8>)> {
    ["computerUse", "codeExecution", "urlContext", "googleSearch"]
        .into_iter()
        .find_map(|tool_name| {
            runtime_gemini_request_body_without_tool(body, tool_name).map(|body| (tool_name, body))
        })
}

enum RuntimeGeminiPrecommitPeek {
    Committed {
        response: reqwest::blocking::Response,
        prefix: Vec<u8>,
    },
    RetryableInvalid {
        response: reqwest::blocking::Response,
        prefix: Vec<u8>,
        reason: String,
    },
}

#[derive(Default)]
struct RuntimeGeminiPrecommitProbe {
    visible_output: bool,
    reasoning_output: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RuntimeGeminiPrecommitDecision {
    Continue,
    Commit,
    RetryableInvalid(String),
}

fn runtime_gemini_response_is_sse(response: &reqwest::blocking::Response) -> bool {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"))
}

fn runtime_gemini_peek_stream_for_retry(
    mut response: reqwest::blocking::Response,
) -> Result<RuntimeGeminiPrecommitPeek> {
    let mut prefix = Vec::new();
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut probe = RuntimeGeminiPrecommitProbe::default();
    let mut byte = [0_u8; 1];

    loop {
        let read = response
            .read(&mut byte)
            .context("failed to read Gemini stream precommit prefix")?;
        if read == 0 {
            if !line.is_empty() {
                let decision =
                    runtime_gemini_precommit_process_line(&line, &mut data_lines, &mut probe);
                if let RuntimeGeminiPrecommitDecision::RetryableInvalid(reason) = decision {
                    return Ok(RuntimeGeminiPrecommitPeek::RetryableInvalid {
                        response,
                        prefix,
                        reason,
                    });
                }
                line.clear();
            }
            if !data_lines.is_empty() {
                match runtime_gemini_precommit_decision_for_data_lines(&data_lines, &mut probe) {
                    RuntimeGeminiPrecommitDecision::Commit => {
                        return Ok(RuntimeGeminiPrecommitPeek::Committed { response, prefix });
                    }
                    RuntimeGeminiPrecommitDecision::RetryableInvalid(reason) => {
                        return Ok(RuntimeGeminiPrecommitPeek::RetryableInvalid {
                            response,
                            prefix,
                            reason,
                        });
                    }
                    RuntimeGeminiPrecommitDecision::Continue => {}
                }
            }
            let reason = if probe.visible_output || probe.reasoning_output {
                return Ok(RuntimeGeminiPrecommitPeek::Committed { response, prefix });
            } else {
                "gemini_empty_response".to_string()
            };
            return Ok(RuntimeGeminiPrecommitPeek::RetryableInvalid {
                response,
                prefix,
                reason,
            });
        }

        prefix.push(byte[0]);
        line.push(byte[0]);
        if prefix.len() >= RUNTIME_GEMINI_PRECOMMIT_PEEK_LIMIT {
            return Ok(RuntimeGeminiPrecommitPeek::Committed { response, prefix });
        }
        if byte[0] != b'\n' {
            continue;
        }

        match runtime_gemini_precommit_process_line(&line, &mut data_lines, &mut probe) {
            RuntimeGeminiPrecommitDecision::Commit => {
                return Ok(RuntimeGeminiPrecommitPeek::Committed { response, prefix });
            }
            RuntimeGeminiPrecommitDecision::RetryableInvalid(reason) => {
                return Ok(RuntimeGeminiPrecommitPeek::RetryableInvalid {
                    response,
                    prefix,
                    reason,
                });
            }
            RuntimeGeminiPrecommitDecision::Continue => {}
        }
        line.clear();
    }
}

fn runtime_gemini_precommit_process_line(
    line: &[u8],
    data_lines: &mut Vec<String>,
    probe: &mut RuntimeGeminiPrecommitProbe,
) -> RuntimeGeminiPrecommitDecision {
    let line = String::from_utf8_lossy(line);
    let line = line.trim_end_matches(['\r', '\n']);
    if line.trim().is_empty() {
        if data_lines.is_empty() {
            return RuntimeGeminiPrecommitDecision::Continue;
        }
        let decision = runtime_gemini_precommit_decision_for_data_lines(data_lines, probe);
        data_lines.clear();
        return decision;
    }
    let Some(data) = line.strip_prefix("data:") else {
        return RuntimeGeminiPrecommitDecision::Continue;
    };
    data_lines.push(data.trim_start().to_string());
    RuntimeGeminiPrecommitDecision::Continue
}

fn runtime_gemini_precommit_decision_for_data_lines(
    data_lines: &[String],
    probe: &mut RuntimeGeminiPrecommitProbe,
) -> RuntimeGeminiPrecommitDecision {
    let data = data_lines.join("\n");
    let trimmed = data.trim();
    if trimmed == "[DONE]" {
        return if probe.visible_output || probe.reasoning_output {
            RuntimeGeminiPrecommitDecision::Commit
        } else {
            RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
        };
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return runtime_gemini_precommit_decision_for_value(&value, probe);
    }
    let mut parsed_any = false;
    for line in data_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        parsed_any = true;
        match runtime_gemini_precommit_decision_for_value(&value, probe) {
            RuntimeGeminiPrecommitDecision::Continue => {}
            decision => return decision,
        }
    }
    if parsed_any {
        RuntimeGeminiPrecommitDecision::Continue
    } else {
        RuntimeGeminiPrecommitDecision::Commit
    }
}

fn runtime_gemini_precommit_decision_for_value(
    value: &serde_json::Value,
    probe: &mut RuntimeGeminiPrecommitProbe,
) -> RuntimeGeminiPrecommitDecision {
    let value = runtime_gemini_normalized_response_value(value);
    let value = value.as_ref();
    if value.get("error").is_some() || runtime_gemini_prompt_feedback_failure(value).is_some() {
        return RuntimeGeminiPrecommitDecision::Commit;
    }
    if runtime_gemini_precommit_has_grounding(value) {
        probe.visible_output = true;
        return RuntimeGeminiPrecommitDecision::Commit;
    }
    runtime_gemini_precommit_apply_parts(value, probe);
    if probe.visible_output {
        return RuntimeGeminiPrecommitDecision::Commit;
    }
    let Some(reason) = runtime_gemini_finish_reason(value) else {
        return RuntimeGeminiPrecommitDecision::Continue;
    };
    if runtime_gemini_finish_reason_retryable_invalid(&reason) {
        return RuntimeGeminiPrecommitDecision::RetryableInvalid(reason);
    }
    match reason.as_str() {
        "STOP" if !probe.reasoning_output => {
            RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
        }
        _ => RuntimeGeminiPrecommitDecision::Commit,
    }
}

fn runtime_gemini_precommit_apply_parts(
    value: &serde_json::Value,
    probe: &mut RuntimeGeminiPrecommitProbe,
) {
    let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    for part in parts {
        if part.get("functionCall").is_some()
            || runtime_gemini_media_content_item_from_part(part).is_some()
            || runtime_gemini_text_from_special_part(part).is_some()
        {
            probe.visible_output = true;
            return;
        }
        let Some(text) = part.get("text").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        if part
            .get("thought")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            probe.reasoning_output = true;
        } else {
            probe.visible_output = true;
            return;
        }
    }
}

fn runtime_gemini_precommit_has_grounding(value: &serde_json::Value) -> bool {
    value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("groundingMetadata"))
        .is_some_and(|metadata| {
            metadata
                .get("webSearchQueries")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|queries| !queries.is_empty())
                || metadata
                    .get("groundingChunks")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|chunks| !chunks.is_empty())
        })
}

fn runtime_gemini_auth_attempts(
    auth: &RuntimeGeminiProviderAuth,
    shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
) -> Result<Vec<RuntimeGeminiSelectedAuth>> {
    match auth {
        RuntimeGeminiProviderAuth::ApiKeys { api_keys } => {
            let attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys)
                .into_iter()
                .map(|(label, api_key)| RuntimeGeminiSelectedAuth {
                    profile_name: label,
                    auth: RuntimeGeminiAuth::ApiKey {
                        api_key: api_key.to_string(),
                    },
                    hard_affinity: api_keys.len() <= 1,
                })
                .collect::<Vec<_>>();
            if attempts.is_empty() {
                bail!("Gemini API-key pool is empty");
            }
            Ok(attempts)
        }
        RuntimeGeminiProviderAuth::OAuthProfiles { profiles } => {
            let pool = shared
                .gemini_oauth_pool
                .as_ref()
                .context("Gemini OAuth pool was not initialized")?;
            pool.select_attempts(body, profiles)
        }
    }
}

pub(super) fn runtime_gemini_live_auth_attempts(
    auth: &RuntimeGeminiProviderAuth,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Result<Vec<(String, RuntimeGeminiAuth)>> {
    Ok(runtime_gemini_auth_attempts(auth, shared, b"{}")?
        .into_iter()
        .map(|selected| (selected.profile_name, selected.auth))
        .collect())
}

impl RuntimeGeminiOAuthPool {
    pub(super) fn spawn_quota_refresh(&self, log_path: PathBuf) {
        let profiles = match self.state.lock() {
            Ok(state) => state.profiles.clone(),
            Err(_) => return,
        };
        if profiles.is_empty() {
            return;
        }
        let pool = self.clone();
        let code_assist_endpoint = gemini_code_assist_endpoint();
        spawn_runtime_background_worker_or_log(
            "prodex-gemini-quota-refresh",
            Some(log_path.clone()),
            move || {
                for profile in profiles {
                    let result = fetch_gemini_quota_with_code_assist_endpoint(
                        &profile.codex_home,
                        profile.project_id.as_deref(),
                        &code_assist_endpoint,
                    );
                    match result {
                        Ok(info) => {
                            let headers = runtime_gemini_quota_codex_headers(
                                &profile.profile_name,
                                profile.email.as_deref(),
                                &info,
                            );
                            pool.remember_quota_headers(&profile.profile_name, headers);
                            runtime_proxy_log_to_path(
                                &log_path,
                                &runtime_proxy_structured_log_message(
                                    "local_rewrite_gemini_quota_status_ready",
                                    [
                                        runtime_proxy_log_field(
                                            "profile",
                                            profile.profile_name.as_str(),
                                        ),
                                        runtime_proxy_log_field(
                                            "buckets",
                                            info.buckets.len().to_string(),
                                        ),
                                    ],
                                ),
                            );
                        }
                        Err(err) => {
                            runtime_proxy_log_to_path(
                                &log_path,
                                &runtime_proxy_structured_log_message(
                                    "local_rewrite_gemini_quota_status_unavailable",
                                    [
                                        runtime_proxy_log_field(
                                            "profile",
                                            profile.profile_name.as_str(),
                                        ),
                                        runtime_proxy_log_field("error", err.to_string()),
                                    ],
                                ),
                            );
                        }
                    }
                }
            },
        );
    }

    pub(super) fn quota_headers_for_profile(&self, profile_name: &str) -> Vec<(String, String)> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.quota_headers.get(profile_name).cloned())
            .unwrap_or_default()
    }

    fn remember_quota_headers(&self, profile_name: &str, headers: Vec<(String, String)>) {
        if headers.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            state
                .quota_headers
                .insert(profile_name.to_string(), headers);
        }
    }

    fn remember_model_cooldown(&self, profile_name: &str, model: &str, cooldown_ms: u64) {
        if profile_name.trim().is_empty() || model.trim().is_empty() || cooldown_ms == 0 {
            return;
        }
        let until = runtime_gemini_now_ms().saturating_add(cooldown_ms);
        if let Ok(mut state) = self.state.lock() {
            state.remember_model_cooldown_until(profile_name, model, until);
        }
    }

    fn select_attempts(
        &self,
        body: &[u8],
        fallback_profiles: &[RuntimeGeminiOAuthProfileAuth],
    ) -> Result<Vec<RuntimeGeminiSelectedAuth>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Gemini OAuth pool lock poisoned"))?;
        if let Some(profile_name) = state.affinity_profile_for_body(body)
            && let Some(profile) = state.profile_by_name(&profile_name)
        {
            return Ok(vec![RuntimeGeminiSelectedAuth {
                profile_name,
                auth: profile.auth(),
                hard_affinity: true,
            }]);
        }
        let profiles = if state.profiles.is_empty() {
            fallback_profiles.to_vec()
        } else {
            state.profiles.clone()
        };
        if profiles.is_empty() {
            bail!("Gemini OAuth pool is empty");
        }
        let requested_model = runtime_provider_model_from_body(body)
            .unwrap_or_else(|| prodex_cli::SUPER_GEMINI_DEFAULT_MODEL.to_string());
        let model_chain = runtime_provider_model_fallback_chain(
            RuntimeProviderBridgeKind::Gemini,
            &requested_model,
        );
        let now_ms = runtime_gemini_now_ms();
        let start = state.next_index.min(profiles.len().saturating_sub(1));
        state.next_index = (start + 1) % profiles.len();
        let mut attempts = (0..profiles.len())
            .map(|offset| {
                runtime_gemini_oauth_attempt_from_profile(
                    &profiles[(start + offset) % profiles.len()],
                )
            })
            .filter(|selected| {
                state.profile_has_available_model(&selected.profile_name, &model_chain, now_ms)
            })
            .collect::<Vec<_>>();
        if attempts.is_empty() {
            attempts = (0..profiles.len())
                .map(|offset| {
                    runtime_gemini_oauth_attempt_from_profile(
                        &profiles[(start + offset) % profiles.len()],
                    )
                })
                .collect();
        }
        Ok(attempts)
    }
}

fn runtime_gemini_oauth_attempt_from_profile(
    profile: &RuntimeGeminiOAuthProfileAuth,
) -> RuntimeGeminiSelectedAuth {
    RuntimeGeminiSelectedAuth {
        profile_name: profile.profile_name.clone(),
        auth: profile.auth(),
        hard_affinity: false,
    }
}

impl RuntimeGeminiOAuthPoolState {
    fn profile_by_name(&self, profile_name: &str) -> Option<RuntimeGeminiOAuthProfileAuth> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_name == profile_name)
            .cloned()
    }

    fn affinity_profile_for_body(&self, body: &[u8]) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        if let Some(previous_response_id) = value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            && let Some(profile_name) = self.response_profile_bindings.get(previous_response_id)
        {
            return Some(profile_name.clone());
        }
        runtime_gemini_tool_output_call_ids_from_request(&value)
            .into_iter()
            .find_map(|call_id| self.tool_call_profile_bindings.get(&call_id).cloned())
    }

    fn remember_bindings(
        &mut self,
        profile_name: &str,
        response_id: &str,
        tool_call_ids: &[String],
    ) {
        if !response_id.trim().is_empty() {
            self.response_profile_bindings
                .insert(response_id.to_string(), profile_name.to_string());
        }
        for call_id in tool_call_ids {
            if !call_id.trim().is_empty() {
                self.tool_call_profile_bindings
                    .insert(call_id.clone(), profile_name.to_string());
            }
        }
        runtime_gemini_prune_binding_map(
            &mut self.response_profile_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
        runtime_gemini_prune_binding_map(
            &mut self.tool_call_profile_bindings,
            RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT,
        );
    }

    fn remember_model_cooldown_until(&mut self, profile_name: &str, model: &str, until_ms: u64) {
        let key = runtime_gemini_model_cooldown_key(profile_name, model);
        self.model_cooldowns_until.insert(key, until_ms);
        self.model_cooldowns_until
            .retain(|_, cooldown_until| *cooldown_until > runtime_gemini_now_ms());
    }

    fn profile_has_available_model(
        &self,
        profile_name: &str,
        models: &[String],
        now_ms: u64,
    ) -> bool {
        models.iter().any(|model| {
            self.model_cooldowns_until
                .get(&runtime_gemini_model_cooldown_key(profile_name, model))
                .is_none_or(|cooldown_until| *cooldown_until <= now_ms)
        })
    }
}

fn runtime_gemini_binding_recorder(
    pool: &RuntimeGeminiOAuthPool,
    profile_name: String,
) -> RuntimeGeminiBindingRecorder {
    let pool = pool.clone();
    Arc::new(move |response_id, tool_call_ids| {
        if let Ok(mut state) = pool.state.lock() {
            state.remember_bindings(&profile_name, &response_id, &tool_call_ids);
        }
    })
}

fn runtime_gemini_prune_binding_map(map: &mut BTreeMap<String, String>, limit: usize) {
    while map.len() > limit {
        let Some(key) = map.keys().next().cloned() else {
            break;
        };
        map.remove(&key);
    }
}

fn runtime_gemini_model_cooldown_key(profile_name: &str, model: &str) -> String {
    format!("{profile_name}\0{model}")
}

fn runtime_gemini_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn runtime_gemini_should_rotate_after_quota_response(
    status: u16,
    hard_affinity: bool,
    attempt_index: usize,
    attempt_count: usize,
) -> bool {
    runtime_gemini_response_retryable_quota(status)
        && !hard_affinity
        && attempt_index + 1 < attempt_count
}

fn runtime_gemini_invalid_stream_retry_delay_ms(retry_index: usize) -> u64 {
    RUNTIME_GEMINI_INVALID_STREAM_RETRY_BASE_DELAY_MS.saturating_mul(1_u64 << retry_index.min(8))
}

#[cfg(test)]
#[path = "local_rewrite_gemini_tests.rs"]
mod local_rewrite_gemini_tests;
