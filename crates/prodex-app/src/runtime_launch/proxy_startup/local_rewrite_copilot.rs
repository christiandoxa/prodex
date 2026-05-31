use super::local_rewrite::{
    RuntimeLocalRewritePreparedAuth, RuntimeLocalRewriteProviderOptions,
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, runtime_copilot_request_body_with_canonical_model,
    runtime_local_rewrite_upstream_url, send_runtime_local_rewrite_prepared_request,
};
use super::local_rewrite_response::runtime_local_rewrite_buffered_response_from_response;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, RuntimeProviderErrorClass, runtime_provider_error_class,
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
    ApiKey {
        api_key: String,
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
    let body = runtime_copilot_request_body_with_canonical_model(&body);
    let upstream_url = runtime_local_rewrite_upstream_url(
        &shared.upstream_base_url,
        &shared.mount_path,
        &request.path_and_query,
    );
    let responses_route = path_without_query(&request.path_and_query).ends_with("/responses");
    let attempts = runtime_copilot_auth_attempts(auth, shared.copilot_oauth_pool.as_ref(), &body)?;
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
        let binding_recorder = shared
            .copilot_oauth_pool
            .as_ref()
            .map(|pool| runtime_copilot_binding_recorder(pool, selected.profile_name.clone()));
        let copilot_context = responses_route.then(|| RuntimeCopilotRequestContext {
            profile_name: selected.profile_name,
            binding_recorder,
        });
        return Ok(RuntimeLocalRewriteUpstreamResult {
            response: RuntimeLocalRewriteUpstreamResponse::Live(response),
            gemini_context: None,
            copilot_context,
        });
    }
    bail!("no Copilot auth attempts were available")
}

fn runtime_copilot_auth_attempts(
    auth: &RuntimeCopilotProviderAuth,
    pool: Option<&RuntimeCopilotOAuthPool>,
    body: &[u8],
) -> Result<Vec<RuntimeCopilotSelectedAuth>> {
    match auth {
        RuntimeCopilotProviderAuth::ApiKey { api_key } => Ok(vec![RuntimeCopilotSelectedAuth {
            profile_name: "api-key".to_string(),
            api_key: api_key.clone(),
            hard_affinity: true,
        }]),
        RuntimeCopilotProviderAuth::Profiles { profiles } => {
            let pool = pool.context("Copilot OAuth pool was not initialized")?;
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
