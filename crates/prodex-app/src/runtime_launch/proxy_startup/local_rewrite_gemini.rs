use super::gemini_rewrite::{
    RuntimeGeminiAuth, RuntimeGeminiOAuthProfileAuth, RuntimeGeminiProviderAuth,
    RuntimeGeminiTranslatedRequest, runtime_gemini_generate_request_body,
    runtime_gemini_project_id, runtime_gemini_upstream_url,
};
use super::gemini_sse::RuntimeGeminiBindingRecorder;
use super::local_rewrite::{
    RuntimeLocalRewritePreparedAuth, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, send_runtime_local_rewrite_prepared_request,
};
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use crate::{RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result, bail};
use runtime_proxy_crate::{
    extract_runtime_proxy_quota_message, path_without_query, runtime_proxy_log_field,
    runtime_proxy_structured_log_message,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const RUNTIME_GEMINI_PROVIDER_BINDING_LIMIT: usize = 4096;
const RUNTIME_GEMINI_RETRY_AFTER_CAP_MS: u64 = 300_000;
const RUNTIME_GEMINI_CLOUD_CODE_DEFAULT_RETRY_MS: u64 = 10_000;
const RUNTIME_GEMINI_PER_MINUTE_RETRY_MS: u64 = 60_000;
const RUNTIME_GEMINI_RATE_LIMIT_RETRY_DELAYS_MS: &[u64] = &[
    5_000, 10_000, 20_000, 30_000, 30_000, 30_000, 30_000, 30_000, 30_000,
];

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
    let attempts = runtime_gemini_auth_attempts(auth, shared.gemini_oauth_pool.as_ref(), &body)?;
    'auth_attempts: for (attempt_index, selected) in attempts.iter().enumerate() {
        let translated = if responses_route {
            runtime_gemini_generate_request_body(
                &body,
                &shared.gemini_conversations,
                matches!(selected.auth, RuntimeGeminiAuth::OAuth { .. }),
                runtime_gemini_project_id(&selected.auth),
            )?
        } else {
            RuntimeGeminiTranslatedRequest {
                body: body.clone(),
                messages: Vec::new(),
                model: prodex_cli::SUPER_GEMINI_DEFAULT_MODEL.to_string(),
                stream: false,
            }
        };
        let upstream_url = runtime_gemini_upstream_url(
            &shared.upstream_base_url,
            &selected.auth,
            &translated.model,
            translated.stream,
        );
        let mut rate_limit_retry_index = 0;
        loop {
            let response = send_runtime_local_rewrite_prepared_request(
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
            if runtime_gemini_response_retryable_quota(status) {
                let retry_after = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_string);
                let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
                let quota_blocked = runtime_gemini_buffered_parts_are_quota_blocked(status, &parts);
                if runtime_gemini_should_rotate_after_quota_response(
                    status,
                    selected.hard_affinity,
                    attempt_index,
                    attempts.len(),
                ) && (status == 429 || quota_blocked)
                {
                    runtime_proxy_log(
                        &shared.runtime_shared,
                        runtime_proxy_structured_log_message(
                            "local_rewrite_gemini_quota_rotate",
                            [
                                runtime_proxy_log_field("request", request_id.to_string()),
                                runtime_proxy_log_field("profile", selected.profile_name.as_str()),
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
                    && let Some(delay_ms) = runtime_gemini_retry_delay_ms(
                        retry_after.as_deref(),
                        &parts.body,
                        rate_limit_retry_index,
                    )
                {
                    runtime_proxy_log(
                        &shared.runtime_shared,
                        runtime_proxy_structured_log_message(
                            "local_rewrite_gemini_rate_limit_retry",
                            [
                                runtime_proxy_log_field("request", request_id.to_string()),
                                runtime_proxy_log_field("profile", selected.profile_name.as_str()),
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
                    response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                    gemini_context: None,
                });
            }

            let binding_recorder = shared
                .gemini_oauth_pool
                .as_ref()
                .map(|pool| runtime_gemini_binding_recorder(pool, selected.profile_name.clone()));
            let gemini_context = responses_route.then(|| RuntimeGeminiRequestContext {
                profile_name: selected.profile_name.clone(),
                conversation_messages: translated.messages,
                binding_recorder,
            });
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Live(response),
                gemini_context,
            });
        }
    }

    bail!("no Gemini auth attempts were available")
}

fn runtime_gemini_auth_attempts(
    auth: &RuntimeGeminiProviderAuth,
    pool: Option<&RuntimeGeminiOAuthPool>,
    body: &[u8],
) -> Result<Vec<RuntimeGeminiSelectedAuth>> {
    match auth {
        RuntimeGeminiProviderAuth::ApiKey { api_key } => Ok(vec![RuntimeGeminiSelectedAuth {
            profile_name: "api-key".to_string(),
            auth: RuntimeGeminiAuth::ApiKey {
                api_key: api_key.clone(),
            },
            hard_affinity: true,
        }]),
        RuntimeGeminiProviderAuth::OAuthProfiles { profiles } => {
            let pool = pool.context("Gemini OAuth pool was not initialized")?;
            pool.select_attempts(body, profiles)
        }
    }
}

impl RuntimeGeminiOAuthPool {
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
        let start = state.next_index.min(profiles.len().saturating_sub(1));
        state.next_index = (start + 1) % profiles.len();
        Ok((0..profiles.len())
            .map(|offset| {
                let profile = profiles[(start + offset) % profiles.len()].clone();
                RuntimeGeminiSelectedAuth {
                    profile_name: profile.profile_name.clone(),
                    auth: profile.auth(),
                    hard_affinity: false,
                }
            })
            .collect())
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

fn runtime_gemini_tool_output_call_ids_from_request(value: &serde_json::Value) -> Vec<String> {
    value
        .get("input")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_object)
        .filter(|object| {
            matches!(
                object.get("type").and_then(serde_json::Value::as_str),
                Some("function_call_output" | "mcp_call_output" | "mcp_tool_result")
            )
        })
        .filter_map(|object| {
            ["call_id", "tool_call_id", "id"]
                .into_iter()
                .find_map(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
        })
        .filter(|call_id| !call_id.trim().is_empty())
        .collect()
}

fn runtime_gemini_response_retryable_quota(status: u16) -> bool {
    matches!(status, 403 | 429)
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

fn runtime_gemini_buffered_parts_are_quota_blocked(
    status: u16,
    parts: &RuntimeHeapTrimmedBufferedResponseParts,
) -> bool {
    runtime_gemini_response_retryable_quota(status)
        && (extract_runtime_proxy_quota_message(&parts.body).is_some()
            || runtime_gemini_google_quota_message(&parts.body).is_some())
}

fn runtime_gemini_retry_delay_ms(
    retry_after: Option<&str>,
    body: &[u8],
    retry_index: usize,
) -> Option<u64> {
    let default_ms = *RUNTIME_GEMINI_RATE_LIMIT_RETRY_DELAYS_MS.get(retry_index)?;
    let server_ms = [
        retry_after.and_then(runtime_gemini_retry_after_header_ms),
        runtime_gemini_retry_delay_ms_from_body(body),
    ]
    .into_iter()
    .flatten()
    .max();
    Some(
        server_ms
            .map(|delay_ms| delay_ms.max(default_ms))
            .unwrap_or(default_ms)
            .min(RUNTIME_GEMINI_RETRY_AFTER_CAP_MS),
    )
}

fn runtime_gemini_google_quota_message(body: &[u8]) -> Option<String> {
    runtime_gemini_values_from_body(body)
        .iter()
        .find_map(runtime_gemini_google_quota_message_from_value)
}

fn runtime_gemini_google_quota_message_from_value(value: &serde_json::Value) -> Option<String> {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                let message = object
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .or_else(|| object.get("detail").and_then(serde_json::Value::as_str))
                    .or_else(|| object.get("error").and_then(serde_json::Value::as_str));
                let explicit_quota = ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(runtime_gemini_google_quota_code)
                });
                if explicit_quota {
                    return Some(
                        message
                            .unwrap_or("Gemini account quota was exhausted.")
                            .to_string(),
                    );
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => {
                stack.extend(values);
            }
            _ => {}
        }
    }
    None
}

fn runtime_gemini_google_quota_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "resource_exhausted"
            | "quota_exhausted"
            | "quota_exceeded"
            | "rate_limit_exceeded"
            | "rate_limit_exceeded_error"
    )
}

fn runtime_gemini_body_has_terminal_quota(body: &[u8]) -> bool {
    runtime_gemini_values_from_body(body)
        .iter()
        .any(runtime_gemini_value_has_terminal_quota)
}

fn runtime_gemini_values_from_body(body: &[u8]) -> Vec<serde_json::Value> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        return vec![value];
    }
    let Ok(text) = std::str::from_utf8(body) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("data:"))
        .map(str::trim)
        .filter(|line| !line.is_empty() && *line != "[DONE]")
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect()
}

fn runtime_gemini_retry_after_header_ms(value: &str) -> Option<u64> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .map(|seconds| seconds.saturating_mul(1_000))
        .filter(|delay_ms| *delay_ms > 0)
}

fn runtime_gemini_retry_delay_ms_from_body(body: &[u8]) -> Option<u64> {
    runtime_gemini_values_from_body(body)
        .iter()
        .filter_map(runtime_gemini_retry_delay_ms_from_value)
        .max()
}

fn runtime_gemini_retry_delay_ms_from_value(value: &serde_json::Value) -> Option<u64> {
    let mut best = None;
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(delay_ms) = object
                    .get("retryDelay")
                    .and_then(serde_json::Value::as_str)
                    .and_then(runtime_gemini_duration_ms)
                {
                    runtime_gemini_update_max_ms(&mut best, delay_ms);
                }
                for key in ["message", "detail", "error"] {
                    if let Some(delay_ms) = object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .and_then(runtime_gemini_retry_delay_ms_from_message)
                    {
                        runtime_gemini_update_max_ms(&mut best, delay_ms);
                    }
                }
                if ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(runtime_gemini_google_rate_limit_code)
                }) {
                    runtime_gemini_update_max_ms(
                        &mut best,
                        RUNTIME_GEMINI_CLOUD_CODE_DEFAULT_RETRY_MS,
                    );
                }
                if runtime_gemini_object_mentions_quota_limit(object, "PerMinute") {
                    runtime_gemini_update_max_ms(&mut best, RUNTIME_GEMINI_PER_MINUTE_RETRY_MS);
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    best
}

fn runtime_gemini_value_has_terminal_quota(value: &serde_json::Value) -> bool {
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(object) => {
                if ["status", "code", "reason"].into_iter().any(|key| {
                    object
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(runtime_gemini_google_terminal_quota_code)
                }) {
                    return true;
                }
                if runtime_gemini_object_mentions_quota_limit(object, "PerDay")
                    || runtime_gemini_object_mentions_quota_limit(object, "Daily")
                {
                    return true;
                }
                stack.extend(object.values());
            }
            serde_json::Value::Array(values) => stack.extend(values),
            _ => {}
        }
    }
    false
}

fn runtime_gemini_update_max_ms(best: &mut Option<u64>, candidate: u64) {
    if candidate == 0 {
        return;
    }
    *best = Some(best.map_or(candidate, |current| current.max(candidate)));
}

fn runtime_gemini_duration_ms(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Some(number) = value.strip_suffix("ms") {
        return runtime_gemini_parse_positive_float(number).map(|millis| {
            let millis = millis.ceil();
            if millis > u64::MAX as f64 {
                u64::MAX
            } else {
                millis as u64
            }
        });
    }
    if let Some(number) = value.strip_suffix('s') {
        return runtime_gemini_parse_positive_float(number).map(|seconds| {
            let millis = (seconds * 1_000.0).ceil();
            if millis > u64::MAX as f64 {
                u64::MAX
            } else {
                millis as u64
            }
        });
    }
    None
}

fn runtime_gemini_parse_positive_float(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn runtime_gemini_retry_delay_ms_from_message(message: &str) -> Option<u64> {
    let lower = message.to_ascii_lowercase();
    ["please retry in ", "suggested retry after "]
        .into_iter()
        .find_map(|marker| {
            let start = lower.find(marker)? + marker.len();
            runtime_gemini_duration_token_ms(&lower[start..])
        })
}

fn runtime_gemini_duration_token_ms(value: &str) -> Option<u64> {
    let value = value.trim_start();
    let number_len = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .map(char::len_utf8)
        .sum::<usize>();
    if number_len == 0 {
        return None;
    }
    let number = &value[..number_len];
    let suffix = &value[number_len..];
    if suffix.starts_with("ms") {
        runtime_gemini_duration_ms(&format!("{number}ms"))
    } else if suffix.starts_with('s') {
        runtime_gemini_duration_ms(&format!("{number}s"))
    } else {
        None
    }
}

fn runtime_gemini_object_mentions_quota_limit(
    object: &serde_json::Map<String, serde_json::Value>,
    needle: &str,
) -> bool {
    object
        .get("quotaId")
        .or_else(|| object.get("quota_limit"))
        .or_else(|| object.get("quotaLimit"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|quota| quota.contains(needle))
        || object
            .get("metadata")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|metadata| runtime_gemini_object_mentions_quota_limit(metadata, needle))
}

fn runtime_gemini_google_rate_limit_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "rate_limit_exceeded" | "rate_limit_exceeded_error"
    )
}

fn runtime_gemini_google_terminal_quota_code(code: &str) -> bool {
    matches!(
        code.trim().to_ascii_lowercase().as_str(),
        "quota_exhausted" | "insufficient_g1_credits_balance" | "insufficient_quota"
    )
}

pub(super) fn runtime_gemini_remember_bindings_from_responses_body(
    recorder: Option<&RuntimeGeminiBindingRecorder>,
    body: &[u8],
) {
    let Some(recorder) = recorder else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return;
    };
    let response_id = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let tool_call_ids = runtime_gemini_tool_call_ids_from_responses_value(&value);
    if !response_id.trim().is_empty() || !tool_call_ids.is_empty() {
        recorder(response_id, tool_call_ids);
    }
}

fn runtime_gemini_tool_call_ids_from_responses_value(value: &serde_json::Value) -> Vec<String> {
    value
        .get("output")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_object)
        .filter(|object| {
            object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| kind == "function_call")
        })
        .filter_map(|object| {
            object
                .get("call_id")
                .or_else(|| object.get("id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .filter(|call_id| !call_id.trim().is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gemini_profile(profile_name: &str) -> RuntimeGeminiOAuthProfileAuth {
        RuntimeGeminiOAuthProfileAuth {
            profile_name: profile_name.to_string(),
            access_token: format!("token-{profile_name}"),
            project_id: Some(format!("project-{profile_name}")),
        }
    }

    fn gemini_pool(profile_names: &[&str]) -> RuntimeGeminiOAuthPool {
        RuntimeGeminiOAuthPool {
            state: Arc::new(Mutex::new(RuntimeGeminiOAuthPoolState {
                profiles: profile_names
                    .iter()
                    .map(|profile_name| gemini_profile(profile_name))
                    .collect(),
                next_index: 0,
                response_profile_bindings: BTreeMap::new(),
                tool_call_profile_bindings: BTreeMap::new(),
            })),
        }
    }

    #[test]
    fn gemini_oauth_pool_rotates_fresh_requests() {
        let pool = gemini_pool(&["alpha", "beta"]);
        let body = serde_json::to_vec(&serde_json::json!({"input": "hi"})).unwrap();

        let first = pool.select_attempts(&body, &[]).unwrap();
        let second = pool.select_attempts(&body, &[]).unwrap();

        assert_eq!(first[0].profile_name, "alpha");
        assert_eq!(first[1].profile_name, "beta");
        assert!(!first[0].hard_affinity);
        assert_eq!(second[0].profile_name, "beta");
        assert_eq!(second[1].profile_name, "alpha");
    }

    #[test]
    fn gemini_oauth_pool_preserves_previous_response_affinity() {
        let pool = gemini_pool(&["alpha", "beta"]);
        pool.state
            .lock()
            .unwrap()
            .remember_bindings("beta", "resp_1", &[]);
        let body =
            serde_json::to_vec(&serde_json::json!({"previous_response_id": "resp_1"})).unwrap();

        let attempts = pool.select_attempts(&body, &[]).unwrap();

        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].profile_name, "beta");
        assert!(attempts[0].hard_affinity);
    }

    #[test]
    fn gemini_oauth_pool_preserves_tool_output_affinity() {
        let pool = gemini_pool(&["alpha", "beta"]);
        pool.state
            .lock()
            .unwrap()
            .remember_bindings("beta", "resp_1", &["call_1".to_string()]);
        let body = serde_json::to_vec(&serde_json::json!({
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "done"
            }]
        }))
        .unwrap();

        let attempts = pool.select_attempts(&body, &[]).unwrap();

        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].profile_name, "beta");
        assert!(attempts[0].hard_affinity);
    }

    #[test]
    fn gemini_binding_recorder_reads_responses_body() {
        let captured = Arc::new(Mutex::new(None::<(String, Vec<String>)>));
        let captured_for_recorder = Arc::clone(&captured);
        let recorder: RuntimeGeminiBindingRecorder = Arc::new(move |response_id, call_ids| {
            *captured_for_recorder.lock().unwrap() = Some((response_id, call_ids));
        });
        let body = serde_json::to_vec(&serde_json::json!({
            "id": "resp_1",
            "output": [{
                "type": "function_call",
                "call_id": "call_1",
                "name": "shell",
                "arguments": "{}"
            }]
        }))
        .unwrap();

        runtime_gemini_remember_bindings_from_responses_body(Some(&recorder), &body);

        let (response_id, call_ids) = captured.lock().unwrap().clone().unwrap();
        assert_eq!(response_id, "resp_1");
        assert_eq!(call_ids, vec!["call_1"]);
    }

    #[test]
    fn gemini_google_resource_exhausted_is_quota_blocked() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Quota exceeded for quota metric.",
                "status": "RESOURCE_EXHAUSTED"
            }
        }))
        .unwrap();

        assert_eq!(
            runtime_gemini_google_quota_message(&body).as_deref(),
            Some("Quota exceeded for quota metric.")
        );
    }

    #[test]
    fn gemini_quota_rotation_predicate_respects_affinity_and_attempt_budget() {
        assert!(runtime_gemini_should_rotate_after_quota_response(
            429, false, 0, 2
        ));
        assert!(!runtime_gemini_should_rotate_after_quota_response(
            429, false, 0, 1
        ));
        assert!(!runtime_gemini_should_rotate_after_quota_response(
            429, true, 0, 2
        ));
        assert!(!runtime_gemini_should_rotate_after_quota_response(
            429, false, 1, 2
        ));
        assert!(!runtime_gemini_should_rotate_after_quota_response(
            500, false, 0, 2
        ));
    }

    #[test]
    fn gemini_rate_limit_retry_delay_uses_defaults_and_retry_after_cap() {
        assert_eq!(runtime_gemini_retry_delay_ms(None, b"", 0), Some(5_000));
        assert_eq!(runtime_gemini_retry_delay_ms(None, b"", 1), Some(10_000));
        assert_eq!(
            runtime_gemini_retry_delay_ms(Some("3"), b"", 0),
            Some(5_000)
        );
        assert_eq!(
            runtime_gemini_retry_delay_ms(Some("99"), b"", 0),
            Some(99_000)
        );
        assert_eq!(
            runtime_gemini_retry_delay_ms(Some("999"), b"", 0),
            Some(300_000)
        );
        assert_eq!(
            runtime_gemini_retry_delay_ms(Some("bad"), b"", 8),
            Some(30_000)
        );
        assert_eq!(runtime_gemini_retry_delay_ms(None, b"", 9), None);
    }

    #[test]
    fn gemini_rate_limit_retry_delay_reads_google_retry_info_from_json() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Resource exhausted, please try again later.",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.RetryInfo",
                    "retryDelay": "34.074824224s"
                }]
            }
        }))
        .unwrap();

        assert_eq!(runtime_gemini_retry_delay_ms(None, &body, 0), Some(34_075));
    }

    #[test]
    fn gemini_rate_limit_retry_delay_reads_google_retry_info_from_sse() {
        let body = concat!(
            "data: {\"error\":{\"code\":429,\"message\":\"Please retry in 12.5s.\",",
            "\"status\":\"RESOURCE_EXHAUSTED\"}}\n\n"
        );

        assert_eq!(
            runtime_gemini_retry_delay_ms(None, body.as_bytes(), 0),
            Some(12_500)
        );
    }

    #[test]
    fn gemini_rate_limit_retry_delay_defaults_cloud_code_rate_limit_to_ten_seconds() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Rate limit exceeded.",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "domain": "cloudcode-pa.googleapis.com",
                    "reason": "RATE_LIMIT_EXCEEDED"
                }]
            }
        }))
        .unwrap();

        assert_eq!(runtime_gemini_retry_delay_ms(None, &body, 0), Some(10_000));
    }

    #[test]
    fn gemini_terminal_quota_body_disables_rate_limit_retry() {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": {
                "code": 429,
                "message": "Quota exhausted.",
                "details": [{
                    "@type": "type.googleapis.com/google.rpc.ErrorInfo",
                    "domain": "cloudcode-pa.googleapis.com",
                    "reason": "QUOTA_EXHAUSTED"
                }]
            }
        }))
        .unwrap();

        assert!(runtime_gemini_body_has_terminal_quota(&body));
    }
}
