//! SSE stream tap reader and side-effect application.

use super::*;

fn apply_runtime_sse_tap_effects(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    request_id: u64,
    prompt_cache_key: Option<&str>,
    model_name: Option<&str>,
    effects: Vec<RuntimeSseTapEffect>,
) {
    for effect in effects {
        match effect {
            RuntimeSseTapEffect::RememberResponseIds {
                response_ids,
                turn_state,
            } => {
                let _ = remember_runtime_response_ids_with_turn_state(
                    shared,
                    profile_name,
                    &response_ids,
                    turn_state.as_deref(),
                    RuntimeRouteKind::Responses,
                );
            }
            RuntimeSseTapEffect::ClearDeadResponseBindings { response_ids } => {
                let _ = clear_runtime_dead_response_bindings(
                    shared,
                    profile_name,
                    &response_ids,
                    "previous_response_not_found_after_commit",
                );
            }
            RuntimeSseTapEffect::LogTokenUsage(token_usage) => {
                log_runtime_token_usage(RuntimeTokenUsageLog {
                    shared,
                    request_id,
                    transport: "http",
                    profile_name,
                    source: "responses_sse",
                    prompt_cache_key,
                    model_name,
                    usage: Some(token_usage),
                });
            }
        }
    }
}

pub(crate) struct RuntimeSseTapReader {
    inner: Box<dyn Read + Send>,
    shared: RuntimeRotationProxyShared,
    profile_name: String,
    prompt_cache_key: Option<String>,
    model_name: Option<String>,
    request_id: u64,
    state: RuntimeSseTapState,
}

pub(crate) struct RuntimeSseTapReaderInit<'a> {
    pub(crate) shared: RuntimeRotationProxyShared,
    pub(crate) profile_name: String,
    pub(crate) prelude: &'a [u8],
    pub(crate) remembered_response_ids: &'a [String],
    pub(crate) request_previous_response_id: Option<&'a str>,
    pub(crate) turn_state: Option<&'a str>,
    pub(crate) request_id: u64,
    pub(crate) prompt_cache_key: Option<&'a str>,
    pub(crate) model_name: Option<&'a str>,
}

impl RuntimeSseTapReader {
    pub(crate) fn new(
        inner: impl Read + Send + 'static,
        init: RuntimeSseTapReaderInit<'_>,
    ) -> Self {
        let RuntimeSseTapReaderInit {
            shared,
            profile_name,
            prelude,
            remembered_response_ids,
            request_previous_response_id,
            turn_state,
            request_id,
            prompt_cache_key,
            model_name,
        } = init;
        let mut state = RuntimeSseTapState::new(RuntimeSseTapStateInit {
            remembered_response_ids,
            request_previous_response_id,
            turn_state,
        });
        let effects = state.observe_chunk(prelude);
        apply_runtime_sse_tap_effects(
            &shared,
            &profile_name,
            request_id,
            prompt_cache_key,
            model_name,
            effects,
        );
        Self {
            inner: Box::new(inner),
            shared,
            profile_name,
            prompt_cache_key: prompt_cache_key.map(str::to_string),
            model_name: model_name.map(str::to_string),
            request_id,
            state,
        }
    }
}

impl Read for RuntimeSseTapReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = match self.inner.read(buf) {
            Ok(read) => read,
            Err(err) => {
                let transport_error =
                    anyhow::Error::new(io::Error::new(err.kind(), err.to_string()));
                note_runtime_profile_transport_failure(
                    &self.shared,
                    &self.profile_name,
                    RuntimeRouteKind::Responses,
                    "sse_read",
                    &transport_error,
                );
                return Err(err);
            }
        };
        if read == 0 {
            let effects = self.state.finish_pending();
            apply_runtime_sse_tap_effects(
                &self.shared,
                &self.profile_name,
                self.request_id,
                self.prompt_cache_key.as_deref(),
                self.model_name.as_deref(),
                effects,
            );
            return Ok(0);
        }
        let effects = self.state.observe_chunk(&buf[..read]);
        apply_runtime_sse_tap_effects(
            &self.shared,
            &self.profile_name,
            self.request_id,
            self.prompt_cache_key.as_deref(),
            self.model_name.as_deref(),
            effects,
        );
        Ok(read)
    }
}
