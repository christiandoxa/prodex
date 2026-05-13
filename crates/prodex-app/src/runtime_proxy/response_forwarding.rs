use super::*;

pub(crate) use runtime_proxy_crate::runtime_token_usage_event_is_loggable;
use runtime_proxy_crate::{
    runtime_buffered_response_metadata, runtime_forward_text_response_headers,
    runtime_response_content_type_is_sse, runtime_response_header_value,
    runtime_sse_forwarding_commit_detail,
};

#[path = "response_forwarding/streaming_writer.rs"]
mod streaming_writer;

pub(crate) use streaming_writer::write_runtime_streaming_response;

pub(super) async fn forward_runtime_proxy_response(
    response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<tiny_http::ResponseBox> {
    let parts = buffer_runtime_proxy_async_response_parts(response, prelude).await?;
    Ok(build_runtime_proxy_response_from_parts(parts))
}

pub(super) async fn forward_runtime_proxy_response_with_limit(
    response: reqwest::Response,
    prelude: Vec<u8>,
    max_bytes: usize,
) -> Result<tiny_http::ResponseBox> {
    let parts =
        buffer_runtime_proxy_async_response_parts_with_limit(response, prelude, max_bytes).await?;
    Ok(build_runtime_proxy_response_from_parts(parts))
}

pub(crate) struct RuntimeResponsesSuccessContext<'a> {
    pub(crate) request_id: u64,
    pub(crate) request_model_name: Option<&'a str>,
    pub(crate) request_previous_response_id: Option<&'a str>,
    pub(crate) request_prompt_cache_key: Option<&'a str>,
    pub(crate) request_session_id: Option<&'a str>,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) turn_state_override: Option<&'a str>,
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) profile_name: &'a str,
    pub(crate) inflight_guard: RuntimeProfileInFlightGuard,
}

pub(crate) async fn prepare_runtime_proxy_responses_success(
    context: RuntimeResponsesSuccessContext<'_>,
    response: reqwest::Response,
) -> Result<RuntimeResponsesAttempt> {
    let RuntimeResponsesSuccessContext {
        request_id,
        request_model_name,
        request_previous_response_id,
        request_prompt_cache_key,
        request_session_id,
        request_turn_state,
        turn_state_override,
        shared,
        profile_name,
        inflight_guard,
    } = context;

    let response_header_turn_state =
        runtime_proxy_header_value(response.headers(), "x-codex-turn-state");
    remember_runtime_successful_previous_response_owner(
        shared,
        profile_name,
        request_previous_response_id,
        RuntimeRouteKind::Responses,
    )?;
    remember_runtime_session_id(
        shared,
        profile_name,
        request_session_id,
        RuntimeRouteKind::Responses,
    )?;
    let response_content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let is_sse = runtime_response_content_type_is_sse(response_content_type);
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http prepare_success profile={profile_name} sse={is_sse} turn_state={:?}",
            response_header_turn_state
        ),
    );
    if !is_sse {
        let buffered_started_at = Instant::now();
        let parts = buffer_runtime_proxy_async_response_parts(response, Vec::new()).await?;
        let response_turn_state = response_header_turn_state
            .or_else(|| turn_state_override.map(str::to_string))
            .or_else(|| extract_runtime_turn_state_from_body_bytes(&parts.body));
        remember_runtime_turn_state(
            shared,
            profile_name,
            response_turn_state.as_deref(),
            RuntimeRouteKind::Responses,
        )?;
        let response_metadata = runtime_buffered_response_metadata(
            parts.status,
            parts
                .headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_slice())),
            parts.body.len(),
        );
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http buffered_response_complete profile={profile_name} phase=responses_unary status={} content_type={} body_bytes={} elapsed_ms={}",
                response_metadata.status,
                response_metadata.content_type.unwrap_or("-"),
                response_metadata.body_bytes,
                buffered_started_at.elapsed().as_millis(),
            ),
        );
        let response_ids = extract_runtime_response_ids_from_body_bytes(&parts.body);
        if !response_ids.is_empty() {
            remember_runtime_response_ids_with_turn_state(
                shared,
                profile_name,
                &response_ids,
                response_turn_state.as_deref(),
                RuntimeRouteKind::Responses,
            )?;
        }
        log_runtime_token_usage(RuntimeTokenUsageLog {
            shared,
            request_id,
            transport: "http",
            profile_name,
            source: "responses_unary",
            prompt_cache_key: request_prompt_cache_key,
            model_name: request_model_name,
            usage: extract_runtime_token_usage_from_body_bytes(&parts.body),
        });
        if !response_ids.is_empty() && response_turn_state.is_some() {
            let _ = release_runtime_compact_lineage(
                shared,
                profile_name,
                request_session_id,
                request_turn_state,
                "response_committed",
            );
        }
        return Ok(RuntimeResponsesAttempt::Success {
            profile_name: profile_name.to_string(),
            response: RuntimeResponsesReply::Buffered(parts),
        });
    }

    let status = response.status().as_u16();
    let headers = runtime_forward_text_response_headers(
        response
            .headers()
            .iter()
            .filter_map(|(name, value)| value.to_str().ok().map(|value| (name.as_str(), value))),
    );

    let prefetch = RuntimePrefetchStream::spawn(
        response,
        Arc::clone(&shared.async_runtime),
        shared.log_path.clone(),
        request_id,
    );
    let (lookahead, prefetch) =
        inspect_runtime_sse_lookahead_async(prefetch, shared.log_path.clone(), request_id).await?;

    let (prelude, response_ids, lookahead_turn_state) = match lookahead {
        RuntimeSseInspection::Commit {
            prelude,
            response_ids,
            turn_state,
        } => {
            let detail = runtime_sse_forwarding_commit_detail(prelude.len(), response_ids.len());
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_commit profile={profile_name} prelude_bytes={} response_ids={}",
                    detail.prelude_bytes, detail.response_id_count
                ),
            );
            (prelude, response_ids, turn_state)
        }
        RuntimeSseInspection::QuotaBlocked(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_quota_blocked profile={profile_name} prelude_bytes={}",
                    prelude.len()
                ),
            );
            return Ok(RuntimeResponsesAttempt::QuotaBlocked {
                profile_name: profile_name.to_string(),
                response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                    status,
                    headers: headers.clone(),
                    body: Box::new(prefetch.into_reader(prelude)?),
                    request_id,
                    profile_name: profile_name.to_string(),
                    log_path: shared.log_path.clone(),
                    shared: shared.clone(),
                    _inflight_guard: Some(inflight_guard),
                }),
            });
        }
        RuntimeSseInspection::PreviousResponseNotFound(prelude) => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http route=responses previous_response_not_found profile={profile_name} stage=sse_prelude prelude_bytes={}",
                    prelude.len()
                ),
            );
            return Ok(RuntimeResponsesAttempt::PreviousResponseNotFound {
                profile_name: profile_name.to_string(),
                response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
                    status,
                    headers: headers.clone(),
                    body: Box::new(prefetch.into_reader(prelude)?),
                    request_id,
                    profile_name: profile_name.to_string(),
                    log_path: shared.log_path.clone(),
                    shared: shared.clone(),
                    _inflight_guard: Some(inflight_guard),
                }),
                turn_state: response_header_turn_state,
            });
        }
    };
    let response_turn_state = response_header_turn_state
        .or_else(|| turn_state_override.map(str::to_string))
        .or(lookahead_turn_state);
    remember_runtime_turn_state(
        shared,
        profile_name,
        response_turn_state.as_deref(),
        RuntimeRouteKind::Responses,
    )?;
    remember_runtime_response_ids_with_turn_state(
        shared,
        profile_name,
        &response_ids,
        response_turn_state.as_deref(),
        RuntimeRouteKind::Responses,
    )?;
    if !response_ids.is_empty() && response_turn_state.is_some() {
        let _ = release_runtime_compact_lineage(
            shared,
            profile_name,
            request_session_id,
            request_turn_state,
            "response_committed",
        );
    }

    let reader = prefetch.into_reader(prelude.clone())?;
    let reader = RuntimeSseTapReader::new(
        reader,
        RuntimeSseTapReaderInit {
            shared: shared.clone(),
            profile_name: profile_name.to_string(),
            prelude: &prelude,
            remembered_response_ids: &response_ids,
            request_previous_response_id,
            turn_state: response_turn_state.as_deref(),
            request_id,
            prompt_cache_key: request_prompt_cache_key,
            model_name: request_model_name,
        },
    );
    let response = RuntimeResponsesAttempt::Success {
        profile_name: profile_name.to_string(),
        response: RuntimeResponsesReply::Streaming(RuntimeStreamingResponse {
            status,
            headers,
            body: Box::new(reader),
            request_id,
            profile_name: profile_name.to_string(),
            log_path: shared.log_path.clone(),
            shared: shared.clone(),
            _inflight_guard: Some(inflight_guard),
        }),
    };
    Ok(response)
}

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

impl Read for RuntimePrefetchReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.finished {
            return Ok(0);
        }

        loop {
            let read = self.pending.read(buf)?;
            if read > 0 {
                return Ok(read);
            }

            let next = if let Some(chunk) = self.backlog.pop_front() {
                Some(chunk)
            } else {
                match self
                    .receiver
                    .recv_timeout(Duration::from_millis(runtime_proxy_stream_idle_timeout_ms()))
                {
                    Ok(chunk) => {
                        if let RuntimePrefetchChunk::Data(bytes) = &chunk {
                            runtime_prefetch_release_queued_bytes(&self.shared, bytes.len());
                        }
                        Some(chunk)
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        self.finished = true;
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "runtime upstream stream idle timed out",
                        ));
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        if let Some((kind, message)) = runtime_prefetch_terminal_error(&self.shared)
                        {
                            self.finished = true;
                            return Err(io::Error::new(kind, message));
                        }
                        None
                    }
                }
            };

            match next {
                Some(RuntimePrefetchChunk::Data(chunk)) => {
                    self.pending = Cursor::new(chunk);
                }
                Some(RuntimePrefetchChunk::End) | None => {
                    self.finished = true;
                    return Ok(0);
                }
                Some(RuntimePrefetchChunk::Error(kind, message)) => {
                    self.finished = true;
                    return Err(io::Error::new(kind, message));
                }
            }
        }
    }
}

impl Drop for RuntimePrefetchReader {
    fn drop(&mut self) {
        self.worker_abort.abort();
    }
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

pub(super) fn runtime_proxy_header_value(
    headers: &reqwest::header::HeaderMap,
    name: &str,
) -> Option<String> {
    runtime_response_header_value(
        headers.iter().filter_map(|(candidate_name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (candidate_name.as_str(), value))
        }),
        name,
    )
}

pub(super) fn runtime_proxy_tungstenite_header_value(
    headers: &tungstenite::http::HeaderMap,
    name: &str,
) -> Option<String> {
    runtime_response_header_value(
        headers.iter().filter_map(|(candidate_name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (candidate_name.as_str(), value))
        }),
        name,
    )
}

#[cfg(test)]
#[path = "../../tests/src/runtime_proxy/response_forwarding.rs"]
mod tests;
