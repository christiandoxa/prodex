use super::*;

pub(super) fn should_skip_runtime_response_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "content-encoding"
            | "content-length"
            | "date"
            | "server"
            | "transfer-encoding"
    )
}

pub(super) fn forward_runtime_proxy_response(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    prelude: Vec<u8>,
) -> Result<tiny_http::ResponseBox> {
    let parts = buffer_runtime_proxy_async_response_parts(shared, response, prelude)?;
    Ok(build_runtime_proxy_response_from_parts(parts))
}

pub(super) fn forward_runtime_proxy_response_with_limit(
    shared: &RuntimeRotationProxyShared,
    response: reqwest::Response,
    prelude: Vec<u8>,
    max_bytes: usize,
) -> Result<tiny_http::ResponseBox> {
    let parts =
        buffer_runtime_proxy_async_response_parts_with_limit(shared, response, prelude, max_bytes)?;
    Ok(build_runtime_proxy_response_from_parts(parts))
}

pub(crate) struct RuntimeResponsesSuccessContext<'a> {
    pub(crate) request_id: u64,
    pub(crate) request_previous_response_id: Option<&'a str>,
    pub(crate) request_session_id: Option<&'a str>,
    pub(crate) request_turn_state: Option<&'a str>,
    pub(crate) turn_state_override: Option<&'a str>,
    pub(crate) shared: &'a RuntimeRotationProxyShared,
    pub(crate) profile_name: &'a str,
    pub(crate) inflight_guard: RuntimeProfileInFlightGuard,
}

pub(crate) fn prepare_runtime_proxy_responses_success(
    context: RuntimeResponsesSuccessContext<'_>,
    response: reqwest::Response,
) -> Result<RuntimeResponsesAttempt> {
    let RuntimeResponsesSuccessContext {
        request_id,
        request_previous_response_id,
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
    let is_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.contains("text/event-stream"));
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http prepare_success profile={profile_name} sse={is_sse} turn_state={:?}",
            response_header_turn_state
        ),
    );
    if !is_sse {
        let buffered_started_at = Instant::now();
        let parts = buffer_runtime_proxy_async_response_parts(shared, response, Vec::new())?;
        let response_turn_state = response_header_turn_state
            .or_else(|| turn_state_override.map(str::to_string))
            .or_else(|| extract_runtime_turn_state_from_body_bytes(&parts.body));
        remember_runtime_turn_state(
            shared,
            profile_name,
            response_turn_state.as_deref(),
            RuntimeRouteKind::Responses,
        )?;
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http buffered_response_complete profile={profile_name} phase=responses_unary status={} content_type={} body_bytes={} elapsed_ms={}",
                parts.status,
                runtime_buffered_response_content_type(&parts).unwrap_or("-"),
                parts.body.len(),
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
    let mut headers = Vec::new();
    for (name, value) in response.headers() {
        if should_skip_runtime_response_header(name.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            headers.push((name.to_string(), value.to_string()));
        }
    }

    let mut prefetch = RuntimePrefetchStream::spawn(
        response,
        Arc::clone(&shared.async_runtime),
        shared.log_path.clone(),
        request_id,
    );
    let lookahead = inspect_runtime_sse_lookahead(&mut prefetch, &shared.log_path, request_id)?;

    let (prelude, response_ids, lookahead_turn_state) = match lookahead {
        RuntimeSseInspection::Commit {
            prelude,
            response_ids,
            turn_state,
        } => {
            runtime_proxy_log(
                shared,
                format!(
                    "request={request_id} transport=http sse_commit profile={profile_name} prelude_bytes={} response_ids={}",
                    prelude.len(),
                    response_ids.len()
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
                    body: Box::new(prefetch.into_reader(prelude)),
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
                    body: Box::new(prefetch.into_reader(prelude)),
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

    let reader = prefetch.into_reader(prelude.clone());
    let reader = RuntimeSseTapReader::new(
        reader,
        shared.clone(),
        profile_name.to_string(),
        &prelude,
        &response_ids,
        request_previous_response_id,
        response_turn_state.as_deref(),
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

impl RuntimeSseTapState {
    fn consume_chunk_events<F>(&mut self, chunk: &[u8], mut on_event: F)
    where
        F: FnMut(&mut Self, RuntimeParsedSseEvent),
    {
        let mut line = std::mem::take(&mut self.line);
        let mut data_lines = std::mem::take(&mut self.data_lines);
        runtime_sse_consume_chunk(&mut line, &mut data_lines, chunk, |event| {
            on_event(self, event);
        });
        self.line = line;
        self.data_lines = data_lines;
    }

    fn finish_pending_events<F>(&mut self, mut on_event: F)
    where
        F: FnMut(&mut Self, RuntimeParsedSseEvent),
    {
        let mut line = std::mem::take(&mut self.line);
        let mut data_lines = std::mem::take(&mut self.data_lines);
        runtime_sse_finish_pending(&mut line, &mut data_lines, |event| {
            on_event(self, event);
        });
        self.line = line;
        self.data_lines = data_lines;
    }

    fn prime_from_prelude(&mut self, chunk: &[u8]) {
        self.consume_chunk_events(chunk, |state, event| state.observe_prelude_event(event));
    }

    fn observe(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str, chunk: &[u8]) {
        self.consume_chunk_events(chunk, |state, event| {
            state.observe_stream_event(shared, profile_name, event);
        });
    }

    fn finish(&mut self, shared: &RuntimeRotationProxyShared, profile_name: &str) {
        self.finish_pending_events(|state, event| {
            state.observe_stream_event(shared, profile_name, event);
        });
    }

    fn observe_prelude_event(&mut self, event: RuntimeParsedSseEvent) {
        if let Some(turn_state) = event.turn_state {
            self.turn_state = Some(turn_state);
        }
    }

    fn observe_stream_event(
        &mut self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        event: RuntimeParsedSseEvent,
    ) {
        if let Some(turn_state) = event.turn_state {
            self.turn_state = Some(turn_state);
        }
        self.remember_response_ids(
            shared,
            profile_name,
            &event.response_ids,
            RuntimeRouteKind::Responses,
        );
        if event.previous_response_not_found {
            self.clear_dead_chain(shared, profile_name);
        }
    }

    fn remember_response_ids(
        &mut self,
        shared: &RuntimeRotationProxyShared,
        profile_name: &str,
        response_ids: &[String],
        verified_route: RuntimeRouteKind,
    ) {
        let turn_state = self.turn_state.as_deref();
        let mut fresh_ids = Vec::new();
        for response_id in response_ids {
            if self.remembered_response_ids.contains(response_id.as_str()) {
                continue;
            }
            let fresh_id = response_id.clone();
            self.remembered_response_ids.insert(fresh_id.clone());
            if turn_state.is_some() {
                self.response_ids_with_turn_state.insert(fresh_id.clone());
            }
            fresh_ids.push(fresh_id);
        }

        let mut response_ids_needing_turn_state = Vec::new();
        if turn_state.is_some()
            && self.response_ids_with_turn_state.len() < self.remembered_response_ids.len()
        {
            for response_id in &self.remembered_response_ids {
                if self
                    .response_ids_with_turn_state
                    .contains(response_id.as_str())
                {
                    continue;
                }
                let rebound_id = response_id.clone();
                self.response_ids_with_turn_state.insert(rebound_id.clone());
                response_ids_needing_turn_state.push(rebound_id);
            }
        }

        if fresh_ids.is_empty() && response_ids_needing_turn_state.is_empty() {
            return;
        }
        if !fresh_ids.is_empty() {
            let _ = remember_runtime_response_ids_with_turn_state(
                shared,
                profile_name,
                &fresh_ids,
                turn_state,
                verified_route,
            );
        }
        if !response_ids_needing_turn_state.is_empty() {
            let _ = remember_runtime_response_ids_with_turn_state(
                shared,
                profile_name,
                &response_ids_needing_turn_state,
                turn_state,
                verified_route,
            );
        }
    }

    fn clear_dead_chain(&self, shared: &RuntimeRotationProxyShared, profile_name: &str) {
        let mut dead_response_ids = self
            .remembered_response_ids
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        if let Some(previous_response_id) = self.request_previous_response_id.as_deref() {
            dead_response_ids.push(previous_response_id.to_string());
        }
        let _ = clear_runtime_dead_response_bindings(
            shared,
            profile_name,
            &dead_response_ids,
            "previous_response_not_found_after_commit",
        );
    }
}

pub(crate) struct RuntimeSseTapReader {
    inner: Box<dyn Read + Send>,
    shared: RuntimeRotationProxyShared,
    profile_name: String,
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

impl RuntimeSseTapReader {
    pub(crate) fn new(
        inner: impl Read + Send + 'static,
        shared: RuntimeRotationProxyShared,
        profile_name: String,
        prelude: &[u8],
        remembered_response_ids: &[String],
        request_previous_response_id: Option<&str>,
        turn_state: Option<&str>,
    ) -> Self {
        let mut state = RuntimeSseTapState {
            remembered_response_ids: remembered_response_ids.iter().cloned().collect(),
            response_ids_with_turn_state: turn_state
                .map(|_| remembered_response_ids.iter().cloned().collect())
                .unwrap_or_default(),
            turn_state: turn_state.map(str::to_string),
            request_previous_response_id: request_previous_response_id.map(str::to_string),
            ..RuntimeSseTapState::default()
        };
        state.prime_from_prelude(prelude);
        Self {
            inner: Box::new(inner),
            shared,
            profile_name,
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
            self.state.finish(&self.shared, &self.profile_name);
            return Ok(0);
        }
        self.state
            .observe(&self.shared, &self.profile_name, &buf[..read]);
        Ok(read)
    }
}

pub(crate) fn write_runtime_streaming_response(
    writer: Box<dyn Write + Send + 'static>,
    mut response: RuntimeStreamingResponse,
) -> io::Result<()> {
    let mut writer = writer;
    let flush_each_chunk = response.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-type")
            && value.to_ascii_lowercase().contains("text/event-stream")
    });
    let started_at = Instant::now();
    let log_writer_error = |stage: &str,
                            chunk_count: usize,
                            total_bytes: usize,
                            err: &io::Error| {
        runtime_proxy_log_to_path(
            &response.log_path,
            &format!(
                "local_writer_error request={} transport=http profile={} stage={} chunks={} bytes={} elapsed_ms={} error={}",
                response.request_id,
                response.profile_name,
                stage,
                chunk_count,
                total_bytes,
                started_at.elapsed().as_millis(),
                err
            ),
        );
    };
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_start profile={} status={}",
            response.request_id, response.profile_name, response.status
        ),
    );
    let status = reqwest::StatusCode::from_u16(response.status)
        .ok()
        .and_then(|status| status.canonical_reason().map(str::to_string))
        .unwrap_or_else(|| "OK".to_string());
    write!(
        writer,
        "HTTP/1.1 {} {}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n",
        response.status, status
    )
    .map_err(|err| {
        log_writer_error("headers_start", 0, 0, &err);
        err
    })?;
    for (name, value) in &response.headers {
        write!(writer, "{name}: {value}\r\n").map_err(|err| {
            log_writer_error("header_line", 0, 0, &err);
            err
        })?;
    }
    writer.write_all(b"\r\n").inspect_err(|err| {
        log_writer_error("headers_end", 0, 0, err);
    })?;
    writer.flush().inspect_err(|err| {
        log_writer_error("headers_flush", 0, 0, err);
    })?;

    let mut buffer = [0_u8; 8192];
    let mut total_bytes = 0usize;
    let mut chunk_count = 0usize;
    let chunk_context = RuntimeStreamChunkContext {
        request_id: response.request_id,
        log_path: response.log_path.clone(),
        profile_name: response.profile_name.clone(),
        shared: response.shared.clone(),
        started_at: &started_at,
        log_writer_error: &log_writer_error,
        flush_each_chunk,
    };
    loop {
        let read = match response.body.read(&mut buffer) {
            Ok(read) => read,
            Err(err) => {
                runtime_proxy_log_to_path(
                    &response.log_path,
                    &format!(
                        "request={} transport=http stream_read_error profile={} chunks={} bytes={} elapsed_ms={} error={}",
                        response.request_id,
                        response.profile_name,
                        chunk_count,
                        total_bytes,
                        started_at.elapsed().as_millis(),
                        err
                    ),
                );
                let transport_error =
                    anyhow::Error::new(io::Error::new(err.kind(), err.to_string()));
                if is_runtime_proxy_transport_failure(&transport_error) {
                    note_runtime_profile_latency_failure(
                        &response.shared,
                        &response.profile_name,
                        RuntimeRouteKind::Responses,
                        "stream_read_error",
                    );
                }
                return Err(err);
            }
        };
        if read == 0 {
            break;
        }
        if chunk_count == 0
            && runtime_take_fault_injection("PRODEX_RUNTIME_FAULT_STREAM_READ_ERROR_ONCE")
        {
            let err = io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected runtime stream read failure",
            );
            runtime_proxy_log_to_path(
                &response.log_path,
                &format!(
                    "request={} transport=http stream_read_error profile={} chunks={} bytes={} elapsed_ms={} error={}",
                    response.request_id,
                    response.profile_name,
                    chunk_count,
                    total_bytes,
                    started_at.elapsed().as_millis(),
                    err
                ),
            );
            note_runtime_profile_latency_failure(
                &response.shared,
                &response.profile_name,
                RuntimeRouteKind::Responses,
                "stream_read_error",
            );
            return Err(err);
        }
        write_runtime_stream_chunk(
            &mut writer,
            &chunk_context,
            &buffer[..read],
            &mut chunk_count,
            &mut total_bytes,
        )?;
    }
    writer.write_all(b"0\r\n\r\n").inspect_err(|err| {
        log_writer_error("trailer", chunk_count, total_bytes, err);
    })?;
    writer.flush().inspect_err(|err| {
        log_writer_error("trailer_flush", chunk_count, total_bytes, err);
    })?;
    runtime_proxy_log_to_path(
        &response.log_path,
        &format!(
            "request={} transport=http stream_complete profile={} chunks={} bytes={} elapsed_ms={}",
            response.request_id,
            response.profile_name,
            chunk_count,
            total_bytes,
            started_at.elapsed().as_millis()
        ),
    );
    note_runtime_profile_latency_observation(
        &response.shared,
        &response.profile_name,
        RuntimeRouteKind::Responses,
        "stream_complete",
        started_at.elapsed().as_millis() as u64,
    );
    Ok(())
}

struct RuntimeStreamChunkContext<'a> {
    request_id: u64,
    log_path: PathBuf,
    profile_name: String,
    shared: RuntimeRotationProxyShared,
    started_at: &'a Instant,
    log_writer_error: &'a dyn Fn(&str, usize, usize, &io::Error),
    flush_each_chunk: bool,
}

fn write_runtime_stream_chunk(
    writer: &mut Box<dyn Write + Send + 'static>,
    context: &RuntimeStreamChunkContext<'_>,
    chunk: &[u8],
    chunk_count: &mut usize,
    total_bytes: &mut usize,
) -> io::Result<()> {
    let RuntimeStreamChunkContext {
        request_id,
        log_path,
        profile_name,
        shared,
        started_at,
        log_writer_error,
        flush_each_chunk,
    } = context;
    if chunk.is_empty() {
        return Ok(());
    }

    *chunk_count += 1;
    *total_bytes += chunk.len();
    if *chunk_count == 1 {
        runtime_proxy_log_to_path(
            log_path,
            &format!(
                "request={} transport=http first_local_chunk profile={} bytes={} elapsed_ms={}",
                request_id,
                profile_name,
                chunk.len(),
                started_at.elapsed().as_millis()
            ),
        );
        note_runtime_profile_latency_observation(
            shared,
            profile_name,
            RuntimeRouteKind::Responses,
            "ttfb",
            started_at.elapsed().as_millis() as u64,
        );
    }
    write!(writer, "{:X}\r\n", chunk.len()).map_err(|err| {
        log_writer_error("chunk_size", *chunk_count, *total_bytes, &err);
        err
    })?;
    writer.write_all(chunk).inspect_err(|err| {
        log_writer_error("chunk_body", *chunk_count, *total_bytes, err);
    })?;
    writer.write_all(b"\r\n").inspect_err(|err| {
        log_writer_error("chunk_suffix", *chunk_count, *total_bytes, err);
    })?;
    if *flush_each_chunk || *chunk_count == 1 {
        writer.flush().inspect_err(|err| {
            log_writer_error("chunk_flush", *chunk_count, *total_bytes, err);
        })?;
    }
    Ok(())
}

pub(super) fn runtime_proxy_header_value(
    headers: &reqwest::header::HeaderMap,
    name: &str,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn runtime_proxy_tungstenite_header_value(
    headers: &tungstenite::http::HeaderMap,
    name: &str,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SharedBufferWriter {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    struct ChunkedReader {
        chunks: VecDeque<Vec<u8>>,
    }

    impl ChunkedReader {
        fn new(chunks: impl IntoIterator<Item = &'static [u8]>) -> Self {
            Self {
                chunks: chunks
                    .into_iter()
                    .map(|chunk| chunk.to_vec())
                    .collect::<VecDeque<_>>(),
            }
        }
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let Some(mut chunk) = self.chunks.pop_front() else {
                return Ok(0);
            };
            let read = chunk.len().min(buf.len());
            buf[..read].copy_from_slice(&chunk[..read]);
            if read < chunk.len() {
                chunk.drain(..read);
                self.chunks.push_front(chunk);
            }
            Ok(read)
        }
    }

    impl Write for SharedBufferWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes
                .lock()
                .map_err(|_| io::Error::other("test writer buffer is poisoned"))?
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn test_runtime_streaming_shared(log_path: PathBuf) -> RuntimeRotationProxyShared {
        let root = env::temp_dir().join(format!(
            "prodex-response-forwarding-test-{}",
            std::process::id()
        ));
        let paths = AppPaths {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared-codex"),
            legacy_shared_codex_root: root.join("shared"),
            root,
        };

        RuntimeRotationProxyShared {
            async_client: reqwest::Client::new(),
            async_runtime: Arc::new(
                TokioRuntimeBuilder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime"),
            ),
            runtime: Arc::new(Mutex::new(RuntimeRotationState {
                paths,
                state: AppState::default(),
                upstream_base_url: "http://127.0.0.1".to_string(),
                include_code_review: false,
                current_profile: "test".to_string(),
                profile_usage_auth: BTreeMap::new(),
                turn_state_bindings: BTreeMap::new(),
                session_id_bindings: BTreeMap::new(),
                continuation_statuses: RuntimeContinuationStatuses::default(),
                profile_probe_cache: BTreeMap::new(),
                profile_usage_snapshots: BTreeMap::new(),
                profile_retry_backoff_until: BTreeMap::new(),
                profile_transport_backoff_until: BTreeMap::new(),
                profile_route_circuit_open_until: BTreeMap::new(),
                profile_inflight: BTreeMap::new(),
                profile_health: BTreeMap::new(),
            })),
            log_path,
            request_sequence: Arc::new(AtomicU64::new(1)),
            state_save_revision: Arc::new(AtomicU64::new(0)),
            local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
            active_request_count: Arc::new(AtomicUsize::new(0)),
            active_request_limit: 8,
            runtime_state_lock_wait_counters:
                RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
            lane_admission: RuntimeProxyLaneAdmission::new(RuntimeProxyLaneLimits {
                responses: 8,
                compact: 8,
                websocket: 8,
                standard: 8,
            }),
        }
    }

    #[test]
    fn sse_tap_reader_observes_split_events_without_event_buffering() {
        let _guard = acquire_test_runtime_lock();
        let log_path = env::temp_dir().join(format!(
            "prodex-response-forwarding-tap-test-{}.log",
            std::process::id()
        ));
        let shared = test_runtime_streaming_shared(log_path);
        let chunks: [&'static [u8]; 4] = [
            b"data: {\"type\":\"response.created\",\"response_id\":\"resp-",
            b"split\"}\r\n\r\n",
            b"data: {\"type\":\"response.in_progress\",\"turn_state\":\"ts-",
            b"split\"}\r\n\r\n",
        ];
        let mut reader = RuntimeSseTapReader::new(
            ChunkedReader::new(chunks),
            shared.clone(),
            "test".to_string(),
            &[],
            &[],
            None,
            None,
        );

        let mut sink = Vec::new();
        reader
            .read_to_end(&mut sink)
            .expect("tap reader should read");

        let runtime = shared.runtime.lock().expect("runtime state");
        assert_eq!(
            runtime
                .state
                .response_profile_bindings
                .get("resp-split")
                .map(|binding| binding.profile_name.as_str()),
            Some("test"),
        );
        assert_eq!(
            runtime
                .state
                .response_profile_bindings
                .get(&runtime_response_turn_state_lineage_key(
                    "resp-split",
                    "ts-split"
                ))
                .map(|binding| binding.profile_name.as_str()),
            Some("test"),
        );
        assert_eq!(sink, chunks.concat());
    }

    #[test]
    fn streaming_sse_previous_response_error_passes_through_after_commit() {
        let event = concat!(
            "event: response.failed\r\n",
            "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"previous_response_not_found\",\"message\":\"Previous response with id 'resp-123' not found.\"}}}\r\n",
            "\r\n"
        );
        let output = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBufferWriter {
            bytes: Arc::clone(&output),
        };
        let response = RuntimeStreamingResponse {
            status: 200,
            headers: vec![("content-type".to_string(), "text/event-stream".to_string())],
            body: Box::new(Cursor::new(event.as_bytes().to_vec())),
            request_id: 1,
            profile_name: "test".to_string(),
            log_path: env::temp_dir().join(format!(
                "prodex-response-forwarding-test-{}.log",
                std::process::id()
            )),
            shared: test_runtime_streaming_shared(env::temp_dir().join(format!(
                "prodex-response-forwarding-shared-test-{}.log",
                std::process::id()
            ))),
            _inflight_guard: None,
        };

        write_runtime_streaming_response(Box::new(writer), response).expect("stream response");
        let text = String::from_utf8(output.lock().expect("output").clone()).expect("utf8");

        assert!(text.contains("previous_response_not_found"));
        assert!(!text.contains("stale_continuation"));
    }
}
