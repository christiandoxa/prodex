#[cfg(test)]
use super::super::copilot_instructions::{
    RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER, runtime_copilot_workspace_custom_instructions,
};
use super::super::copilot_instructions::{
    runtime_copilot_apply_custom_instructions, runtime_copilot_cached_workspace_custom_instructions,
};
use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, RuntimeDeepSeekTranslatedRequest,
    runtime_chat_compatible_request_body,
};
use super::local_rewrite::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, runtime_local_rewrite_model_selection,
};
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::local_rewrite_search_fallback::{
    RuntimeLocalRewritePreparedSendResult, RuntimeLocalRewriteSearchFallbackRequest,
    send_runtime_local_rewrite_prepared_request_with_chat_search_fallback,
};
use super::local_rewrite_transport::{
    RuntimeLocalRewritePreparedAuth, runtime_copilot_request_body_with_canonical_model,
    runtime_local_rewrite_api_key_attempts, runtime_local_rewrite_upstream_url,
    runtime_openai_standard_provider_upstream_url, send_runtime_local_rewrite_prepared_request,
};
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
    runtime_provider_model_fallback_chain, runtime_provider_request_body_with_model,
    runtime_provider_should_retry_with_next_model,
};
use crate::{RuntimeProxyRequest, runtime_proxy_log};
use anyhow::{Context, Result, bail};
use runtime_proxy_crate::{
    path_without_query, runtime_proxy_log_field, runtime_proxy_structured_log_message,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{BufRead, BufReader, Read, Result as IoResult};
use std::sync::{Arc, Mutex};

const RUNTIME_COPILOT_PROVIDER_BINDING_LIMIT: usize = 4096;

pub(super) type RuntimeCopilotBindingRecorder = Arc<dyn Fn(String) + Send + Sync>;

#[derive(Clone)]
pub(crate) enum RuntimeCopilotProviderAuth {
    ApiKeys {
        api_keys: Vec<String>,
    },
    Profiles {
        profiles: Vec<RuntimeCopilotProfileAuth>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeCopilotProfileAuth {
    pub(crate) profile_name: String,
    pub(crate) api_key: String,
}

#[derive(Clone)]
pub(super) struct RuntimeCopilotOAuthPool {
    state: Arc<Mutex<RuntimeCopilotOAuthPoolState>>,
}

#[derive(Debug)]
struct RuntimeCopilotOAuthPoolState {
    profiles: Vec<RuntimeCopilotProfileAuth>,
    next_index: usize,
    response_profile_bindings: BTreeMap<String, String>,
}

#[derive(Clone)]
struct RuntimeCopilotSelectedAuth {
    profile_name: String,
    api_key: String,
    hard_affinity: bool,
}

#[derive(Clone)]
pub(super) struct RuntimeCopilotRequestContext {
    pub(super) profile_name: String,
    pub(super) binding_recorder: Option<RuntimeCopilotBindingRecorder>,
}

pub(super) fn runtime_copilot_oauth_pool_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<RuntimeCopilotOAuthPool> {
    let RuntimeLocalRewriteProviderOptions::Copilot {
        auth: RuntimeCopilotProviderAuth::Profiles { profiles },
    } = provider
    else {
        return None;
    };
    Some(RuntimeCopilotOAuthPool {
        state: Arc::new(Mutex::new(RuntimeCopilotOAuthPoolState {
            profiles: profiles.clone(),
            next_index: 0,
            response_profile_bindings: BTreeMap::new(),
        })),
    })
}

pub(super) fn send_runtime_copilot_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeCopilotProviderAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let responses_route = path_without_query(&request.path_and_query).ends_with("/responses");
    if responses_route {
        return send_runtime_copilot_responses_request(request_id, request, shared, body, auth);
    }

    let body = runtime_copilot_request_body_with_canonical_model(&body);
    let upstream_url = runtime_local_rewrite_upstream_url(
        &shared.upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
    );
    let attempts = runtime_copilot_auth_attempts(auth, shared, &body)?;
    let attempt_count = attempts.len();
    for (attempt_index, selected) in attempts.into_iter().enumerate() {
        let response = send_runtime_local_rewrite_prepared_request(
            request_id,
            request,
            shared,
            &upstream_url,
            body.clone(),
            RuntimeLocalRewritePreparedAuth::Copilot {
                api_key: &selected.api_key,
            },
        )?;
        let status = response.status().as_u16();
        if status >= 400 && !selected.hard_affinity && attempt_index + 1 < attempt_count {
            let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
            let class = runtime_provider_error_class(
                RuntimeProviderBridgeKind::Copilot,
                status,
                &parts.body,
            );
            if runtime_copilot_should_rotate_after_response(class) {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_copilot_profile_rotate",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                continue;
            }
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            });
        }
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Live(
                RuntimeLocalRewriteLiveResponse::new(response),
            ),
            gemini_context: None,
            copilot_context: None,
        });
    }
    bail!("no Copilot auth attempts were available")
}

fn send_runtime_copilot_responses_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    auth: &RuntimeCopilotProviderAuth,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let attempts = runtime_copilot_auth_attempts(auth, shared, &body)?;
    let attempt_count = attempts.len();
    let model_selection = runtime_local_rewrite_model_selection(
        shared,
        RuntimeProviderBridgeKind::Copilot,
        request,
        &body,
        prodex_cli::SUPER_COPILOT_DEFAULT_MODEL,
    );
    let model_chain = runtime_provider_model_fallback_chain(
        RuntimeProviderBridgeKind::Copilot,
        &model_selection.model,
    );
    let upstream_url = runtime_openai_standard_provider_upstream_url(
        RuntimeProviderBridgeKind::Copilot,
        &shared.upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
    );

    for (attempt_index, selected) in attempts.into_iter().enumerate() {
        for (model_index, model) in model_chain.iter().enumerate() {
            let model_body = runtime_provider_request_body_with_model(&model_selection.body, model);
            let translated = runtime_copilot_responses_chat_request_body(
                &model_body,
                &shared.deepseek_conversations,
            )?;
            if let Ok(mut pending) = shared.deepseek_pending_messages.lock() {
                pending.insert(request_id, translated.messages);
            }
            let send_result =
                send_runtime_local_rewrite_prepared_request_with_chat_search_fallback(
                    RuntimeLocalRewriteSearchFallbackRequest {
                        request_id,
                        request,
                        shared,
                        upstream_url: &upstream_url,
                        body: translated.body,
                        provider_kind: RuntimeProviderBridgeKind::Copilot,
                        auth_label: selected.profile_name.as_str(),
                        model,
                        auth_factory: || RuntimeLocalRewritePreparedAuth::Copilot {
                            api_key: selected.api_key.as_str(),
                        },
                    },
                )?;
            let (status, parts, class) = match send_result {
                RuntimeLocalRewritePreparedSendResult::Live(response) => {
                    return Ok(RuntimeLocalRewriteUpstreamResult {
                        response: RuntimeLocalRewriteUpstreamResponse::Live(
                            RuntimeLocalRewriteLiveResponse::new(response),
                        ),
                        gemini_context: None,
                        copilot_context: Some(runtime_copilot_request_context(
                            shared,
                            selected.profile_name.clone(),
                        )),
                    });
                }
                RuntimeLocalRewritePreparedSendResult::Error {
                    status,
                    parts,
                    class,
                } => (status, parts, class),
            };
            if model_index + 1 < model_chain.len()
                && runtime_provider_should_retry_with_next_model(class)
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_provider_model_fallback",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("provider", "copilot"),
                            runtime_proxy_log_field("auth", selected.profile_name.as_str()),
                            runtime_proxy_log_field("from_model", model.as_str()),
                            runtime_proxy_log_field(
                                "to_model",
                                model_chain[model_index + 1].as_str(),
                            ),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                continue;
            }
            if !selected.hard_affinity
                && attempt_index + 1 < attempt_count
                && runtime_copilot_should_rotate_after_response(class)
            {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    runtime_proxy_structured_log_message(
                        "local_rewrite_copilot_profile_rotate",
                        [
                            runtime_proxy_log_field("request", request_id.to_string()),
                            runtime_proxy_log_field("profile", selected.profile_name.as_str()),
                            runtime_proxy_log_field("status", status.to_string()),
                            runtime_proxy_log_field("class", format!("{class:?}")),
                        ],
                    ),
                );
                break;
            }
            return Ok(RuntimeLocalRewriteUpstreamResult {
                response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
                gemini_context: None,
                copilot_context: None,
            });
        }
    }
    bail!("no Copilot model attempts were available")
}

fn runtime_copilot_responses_chat_request_body(
    body: &[u8],
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeDeepSeekTranslatedRequest> {
    let mut translated = runtime_chat_compatible_request_body(
        body,
        conversations,
        RuntimeProviderBridgeKind::Copilot,
        prodex_cli::SUPER_COPILOT_DEFAULT_MODEL,
        false,
    )?;
    if let Some(instructions) = runtime_copilot_cached_workspace_custom_instructions() {
        runtime_copilot_apply_custom_instructions(&mut translated, instructions)?;
    }
    Ok(translated)
}

fn runtime_copilot_request_context(
    shared: &RuntimeLocalRewriteProxyShared,
    profile_name: String,
) -> RuntimeCopilotRequestContext {
    let binding_recorder = shared
        .copilot_oauth_pool
        .as_ref()
        .map(|pool| runtime_copilot_binding_recorder(pool, profile_name.clone()));
    RuntimeCopilotRequestContext {
        profile_name,
        binding_recorder,
    }
}

fn runtime_copilot_auth_attempts(
    auth: &RuntimeCopilotProviderAuth,
    shared: &RuntimeLocalRewriteProxyShared,
    body: &[u8],
) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
    match auth {
        RuntimeCopilotProviderAuth::ApiKeys { api_keys } => {
            let attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys)
                .into_iter()
                .map(|(label, api_key)| RuntimeCopilotSelectedAuth {
                    profile_name: label,
                    api_key: api_key.to_string(),
                    hard_affinity: api_keys.len() <= 1,
                })
                .collect::<Vec<_>>();
            if attempts.is_empty() {
                bail!("Copilot API-key pool is empty");
            }
            Ok(attempts)
        }
        RuntimeCopilotProviderAuth::Profiles { profiles } => {
            let pool = shared
                .copilot_oauth_pool
                .as_ref()
                .context("Copilot OAuth pool was not initialized")?;
            pool.select_attempts(body, profiles)
        }
    }
}

impl RuntimeCopilotOAuthPool {
    fn select_attempts(
        &self,
        body: &[u8],
        fallback_profiles: &[RuntimeCopilotProfileAuth],
    ) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Copilot OAuth pool lock poisoned"))?;
        if let Some(profile_name) = state.affinity_profile_for_body(body)
            && let Some(profile) = state.profile_by_name(&profile_name)
        {
            return Ok(vec![RuntimeCopilotSelectedAuth {
                profile_name,
                api_key: profile.api_key,
                hard_affinity: true,
            }]);
        }
        let profiles = if state.profiles.is_empty() {
            fallback_profiles.to_vec()
        } else {
            state.profiles.clone()
        };
        if profiles.is_empty() {
            bail!("Copilot OAuth pool is empty");
        }
        let start = state.next_index.min(profiles.len().saturating_sub(1));
        state.next_index = (start + 1) % profiles.len();
        Ok((0..profiles.len())
            .map(|offset| {
                let profile = profiles[(start + offset) % profiles.len()].clone();
                RuntimeCopilotSelectedAuth {
                    profile_name: profile.profile_name,
                    api_key: profile.api_key,
                    hard_affinity: false,
                }
            })
            .collect())
    }
}

impl RuntimeCopilotOAuthPoolState {
    fn profile_by_name(&self, profile_name: &str) -> Option<RuntimeCopilotProfileAuth> {
        self.profiles
            .iter()
            .find(|profile| profile.profile_name == profile_name)
            .cloned()
    }

    fn affinity_profile_for_body(&self, body: &[u8]) -> Option<String> {
        let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
        value
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|response_id| self.response_profile_bindings.get(response_id))
            .cloned()
    }

    fn remember_response_binding(&mut self, profile_name: &str, response_id: &str) {
        if response_id.trim().is_empty() {
            return;
        }
        self.response_profile_bindings
            .insert(response_id.to_string(), profile_name.to_string());
        while self.response_profile_bindings.len() > RUNTIME_COPILOT_PROVIDER_BINDING_LIMIT {
            let Some(key) = self.response_profile_bindings.keys().next().cloned() else {
                break;
            };
            self.response_profile_bindings.remove(&key);
        }
    }
}

fn runtime_copilot_binding_recorder(
    pool: &RuntimeCopilotOAuthPool,
    profile_name: String,
) -> RuntimeCopilotBindingRecorder {
    let pool = pool.clone();
    Arc::new(move |response_id| {
        if let Ok(mut state) = pool.state.lock() {
            state.remember_response_binding(&profile_name, &response_id);
        }
    })
}

fn runtime_copilot_should_rotate_after_response(class: RuntimeProviderErrorClass) -> bool {
    matches!(
        class,
        RuntimeProviderErrorClass::Auth
            | RuntimeProviderErrorClass::Quota
            | RuntimeProviderErrorClass::RateLimit
            | RuntimeProviderErrorClass::Transient
    )
}

pub(super) fn runtime_copilot_remember_bindings_from_responses_body(
    recorder: Option<&RuntimeCopilotBindingRecorder>,
    body: &[u8],
) {
    let Some(recorder) = recorder else {
        return;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return;
    };
    if let Some(response_id) = runtime_copilot_response_id_from_value(&value) {
        recorder(response_id);
    }
}

pub(super) struct RuntimeCopilotResponsesSseBindingReader<R> {
    reader: BufReader<R>,
    pending: VecDeque<u8>,
    event_name: String,
    data: String,
    binding_recorder: Option<RuntimeCopilotBindingRecorder>,
    seen_response_ids: BTreeSet<String>,
    eof_dispatched: bool,
}

impl<R: Read> RuntimeCopilotResponsesSseBindingReader<R> {
    pub(super) fn new(reader: R, binding_recorder: Option<RuntimeCopilotBindingRecorder>) -> Self {
        Self {
            reader: BufReader::new(reader),
            pending: VecDeque::new(),
            event_name: String::new(),
            data: String::new(),
            binding_recorder,
            seen_response_ids: BTreeSet::new(),
            eof_dispatched: false,
        }
    }

    fn fill_pending(&mut self) -> IoResult<bool> {
        let mut line = Vec::new();
        let read = self.reader.read_until(b'\n', &mut line)?;
        if read == 0 {
            if !self.eof_dispatched {
                self.dispatch_event();
                self.eof_dispatched = true;
            }
            return Ok(false);
        }
        self.observe_sse_line(&line);
        self.pending.extend(line);
        Ok(true)
    }

    fn observe_sse_line(&mut self, line: &[u8]) {
        let Ok(text) = std::str::from_utf8(line) else {
            return;
        };
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            self.dispatch_event();
            return;
        }
        if let Some(event_name) = text.strip_prefix("event:") {
            self.event_name = event_name.trim().to_string();
            return;
        }
        if let Some(data) = text.strip_prefix("data:") {
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(data.trim_start());
        }
    }

    fn dispatch_event(&mut self) {
        let data = std::mem::take(&mut self.data);
        self.event_name.clear();
        if data.trim().is_empty() || data.trim() == "[DONE]" {
            return;
        }
        let Some(recorder) = self.binding_recorder.as_ref() else {
            return;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
            return;
        };
        if let Some(response_id) = runtime_copilot_response_id_from_value(&value)
            && self.seen_response_ids.insert(response_id.clone())
        {
            recorder(response_id);
        }
    }
}

impl<R: Read> Read for RuntimeCopilotResponsesSseBindingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.pending.is_empty() {
            if !self.fill_pending()? {
                return Ok(0);
            }
        }
        let mut written = 0;
        while written < buf.len() {
            let Some(byte) = self.pending.pop_front() else {
                break;
            };
            buf[written] = byte;
            written += 1;
        }
        Ok(written)
    }
}

fn runtime_copilot_response_id_from_value(value: &serde_json::Value) -> Option<String> {
    value
        .get("response")
        .and_then(|response| response.get("id"))
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("id").and_then(serde_json::Value::as_str))
        .or_else(|| value.get("response_id").and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|response_id| !response_id.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn copilot_profile(profile_name: &str) -> RuntimeCopilotProfileAuth {
        RuntimeCopilotProfileAuth {
            profile_name: profile_name.to_string(),
            api_key: format!("token-{profile_name}"),
        }
    }

    fn copilot_pool(profile_names: &[&str]) -> RuntimeCopilotOAuthPool {
        RuntimeCopilotOAuthPool {
            state: Arc::new(Mutex::new(RuntimeCopilotOAuthPoolState {
                profiles: profile_names
                    .iter()
                    .map(|profile_name| copilot_profile(profile_name))
                    .collect(),
                next_index: 0,
                response_profile_bindings: BTreeMap::new(),
            })),
        }
    }

    fn conversation_store() -> RuntimeDeepSeekConversationStore {
        Arc::new(Mutex::new(BTreeMap::new()))
    }

    fn temp_copilot_instruction_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "prodex-copilot-instructions-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn copilot_workspace_custom_instructions_reads_github_files_only() {
        let root = temp_copilot_instruction_root("github-only");
        std::fs::create_dir_all(root.join(".github/instructions/nested")).unwrap();
        std::fs::write(
            root.join(".github/copilot-instructions.md"),
            "Prefer concise answers.",
        )
        .unwrap();
        std::fs::write(
            root.join(".github/instructions/nested/review.instructions.md"),
            "Review risky diffs first.",
        )
        .unwrap();
        std::fs::write(root.join("AGENTS.md"), "@/home/doxa/.prodex/private/RTK.md").unwrap();

        let instructions = runtime_copilot_workspace_custom_instructions(&root)
            .unwrap()
            .unwrap();

        assert!(instructions.contains("## .github/copilot-instructions.md"));
        assert!(instructions.contains("Prefer concise answers."));
        assert!(instructions.contains("## .github/instructions/nested/review.instructions.md"));
        assert!(instructions.contains("Review risky diffs first."));
        assert!(!instructions.contains("AGENTS.md"));
        assert!(!instructions.contains(".prodex/private"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn copilot_custom_instructions_merge_into_chat_body() {
        let mut translated = RuntimeDeepSeekTranslatedRequest {
            body: serde_json::to_vec(&serde_json::json!({
                "model": "gpt-5.1-codex",
                "stream": true,
                "messages": [
                    {"role": "system", "content": "Existing system."},
                    {"role": "user", "content": "Hi"}
                ]
            }))
            .unwrap(),
            messages: vec![
                serde_json::json!({"role": "system", "content": "Existing system."}),
                serde_json::json!({"role": "user", "content": "Hi"}),
            ],
        };

        runtime_copilot_apply_custom_instructions(&mut translated, "Prefer Rust tests.").unwrap();

        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let system = body["messages"][0]["content"].as_str().unwrap();
        assert!(system.contains("Existing system."));
        assert!(system.contains(RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER));
        assert!(system.contains("Prefer Rust tests."));
        assert_eq!(
            translated.messages,
            body["messages"].as_array().unwrap().clone()
        );
    }

    #[test]
    fn copilot_responses_bridge_maps_mcp_optional_tools_to_chat_functions() {
        let conversations = conversation_store();
        let request = serde_json::json!({
            "model": "codex",
            "stream": true,
            "input": "compress and inspect the workspace",
            "tools": [
                {
                    "type": "mcp_tool",
                    "name": "mcp__prodex_sqz__sqz_read_file",
                    "description": "Read a file through SQZ.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"}
                        },
                        "required": ["path"]
                    }
                },
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "prodex-sqz",
                    "default_config": {"enabled": false},
                    "configs": {
                        "compress": {"enabled": true},
                        "sqz_read_file": {"enabled": false}
                    }
                }
            ],
            "tool_choice": {
                "type": "mcp_tool",
                "name": "mcp__prodex_sqz__sqz_read_file"
            }
        });

        let translated = runtime_copilot_responses_chat_request_body(
            &serde_json::to_vec(&request).unwrap(),
            &conversations,
        )
        .expect("Copilot Responses request should translate to chat");
        let body: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        let tools = body["tools"].as_array().unwrap();

        assert_eq!(body["model"], prodex_cli::SUPER_COPILOT_DEFAULT_MODEL);
        assert_eq!(tools.len(), 2);
        assert_eq!(
            tools[0]["function"]["name"],
            "mcp__prodex_sqz__sqz_read_file"
        );
        assert_eq!(tools[0]["function"]["parameters"]["required"][0], "path");
        assert_eq!(tools[1]["function"]["name"], "mcp__prodex_sqz__compress");
        assert_eq!(
            body["tool_choice"]["function"]["name"],
            "mcp__prodex_sqz__sqz_read_file"
        );
    }

    #[test]
    fn copilot_bridge_stream_records_response_id_and_restores_mcp_namespace() {
        let captured = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured_for_recorder = Arc::clone(&captured);
        let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
            captured_for_recorder.lock().unwrap().push(response_id);
        });
        let chat_stream = concat!(
            "data: {\"id\":\"chatcmpl_copilot_1\",\"model\":\"gpt-5.1-codex\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_sqz_1\",\"type\":\"function\",\"function\":{\"name\":\"mcp__prodex_sqz__compress\",\"arguments\":\"{}\"}}]}}]}\n\n",
            "data: {\"id\":\"chatcmpl_copilot_1\",\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let chat_reader = super::super::deepseek_rewrite::RuntimeDeepSeekChatSseReader::new(
            Cursor::new(chat_stream),
            11,
            Vec::new(),
            conversation_store(),
        );
        let mut reader = RuntimeCopilotResponsesSseBindingReader::new(chat_reader, Some(recorder));
        let mut output = String::new();

        reader.read_to_string(&mut output).unwrap();

        assert!(output.contains("\"namespace\":\"mcp__prodex_sqz\""));
        assert!(output.contains("\"name\":\"compress\""));
        assert_eq!(captured.lock().unwrap().as_slice(), ["chatcmpl_copilot_1"]);
    }

    #[test]
    fn copilot_oauth_pool_rotates_fresh_requests() {
        let pool = copilot_pool(&["alpha", "beta"]);
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
    fn copilot_oauth_pool_preserves_previous_response_affinity() {
        let pool = copilot_pool(&["alpha", "beta"]);
        pool.state
            .lock()
            .unwrap()
            .remember_response_binding("beta", "resp_1");
        let body =
            serde_json::to_vec(&serde_json::json!({"previous_response_id": "resp_1"})).unwrap();

        let attempts = pool.select_attempts(&body, &[]).unwrap();

        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].profile_name, "beta");
        assert!(attempts[0].hard_affinity);
    }

    #[test]
    fn copilot_sse_binding_reader_preserves_bytes_and_records_response_id() {
        let captured = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured_for_recorder = Arc::clone(&captured);
        let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
            captured_for_recorder.lock().unwrap().push(response_id);
        });
        let stream = concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\"}}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"response_id\":\"resp_1\",\"delta\":\"hi\"}\n\n",
            "data: [DONE]\n\n",
        );
        let mut reader =
            RuntimeCopilotResponsesSseBindingReader::new(Cursor::new(stream), Some(recorder));
        let mut output = String::new();

        reader.read_to_string(&mut output).unwrap();

        assert_eq!(output, stream);
        assert_eq!(captured.lock().unwrap().as_slice(), ["resp_1"]);
    }

    #[test]
    fn copilot_binding_recorder_reads_buffered_responses_body() {
        let captured = Arc::new(Mutex::new(None::<String>));
        let captured_for_recorder = Arc::clone(&captured);
        let recorder: RuntimeCopilotBindingRecorder = Arc::new(move |response_id| {
            *captured_for_recorder.lock().unwrap() = Some(response_id);
        });
        let body = serde_json::to_vec(&serde_json::json!({"id": "resp_1"})).unwrap();

        runtime_copilot_remember_bindings_from_responses_body(Some(&recorder), &body);

        assert_eq!(captured.lock().unwrap().as_deref(), Some("resp_1"));
    }
}
