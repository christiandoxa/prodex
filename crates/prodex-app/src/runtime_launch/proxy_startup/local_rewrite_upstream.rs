use super::deepseek_rewrite::RuntimeDeepSeekPendingRequest;
use super::local_rewrite::{
    RUNTIME_LOCAL_REWRITE_PROFILE, RuntimeLocalRewriteAsyncResponse,
    RuntimeLocalRewriteProviderOptions, RuntimeLocalRewriteProxyShared,
};
use super::local_rewrite_anthropic::send_runtime_anthropic_upstream_request;
use super::local_rewrite_application_data_plane::RuntimeGatewayApplicationProviderDispatch;
use super::local_rewrite_copilot::{
    RuntimeCopilotBindingRecorder, RuntimeCopilotRequestContext,
    send_runtime_copilot_upstream_request,
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
    runtime_harness_log_provider_policy, runtime_provider_error_class,
    runtime_provider_model_from_body,
};
use crate::{
    RuntimeHeapTrimmedBufferedResponseParts, RuntimeProxyRequest, RuntimeRouteKind,
    prepare_runtime_smart_context_http_body, runtime_proxy_log,
};
use anyhow::Result;
use prodex_provider_core::{
    ProviderEndpoint, ProviderErrorClass, ProviderId, RuntimeProviderBindingIdentity,
};
use prodex_provider_spi::runtime_provider_binding_identity_from_secret_ref;
use prodex_state::ResponseProfileBinding;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::{self, Cursor, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver};

const RUNTIME_LOCAL_REWRITE_STREAM_CHUNK_BYTES: usize = 64 * 1024;

pub(super) struct RuntimeLocalRewriteUpstreamResult {
    pub(super) response: RuntimeLocalRewriteUpstreamResponse,
    pub(super) gemini_context: Option<RuntimeGeminiRequestContext>,
    pub(super) copilot_context: Option<RuntimeCopilotRequestContext>,
}

impl RuntimeLocalRewriteUpstreamResult {
    pub(super) fn status(&self) -> u16 {
        match &self.response {
            RuntimeLocalRewriteUpstreamResponse::Live(live) => live.status,
            RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => parts.status,
            RuntimeLocalRewriteUpstreamResponse::Streaming(streaming) => streaming.status,
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub(super) enum RuntimeLocalRewriteUpstreamResponse {
    Live(RuntimeLocalRewriteLiveResponse),
    Buffered(RuntimeHeapTrimmedBufferedResponseParts),
    Streaming(RuntimeLocalRewriteStreamingResponse),
}

pub(super) struct RuntimeLocalRewriteLiveResponse {
    pub(super) status: u16,
    pub(super) headers: reqwest::header::HeaderMap,
    pub(super) body: Option<RuntimeLocalRewriteLiveBody>,
    pub(super) prefix: Vec<u8>,
    pub(super) upstream_eof: bool,
    pub(super) native_anthropic_messages: bool,
    pub(super) chat_compatible_request: Option<RuntimeDeepSeekPendingRequest>,
    pub(super) accepted_binding_recorder: Option<RuntimeCopilotBindingRecorder>,
    pub(super) accepted_binding: Option<RuntimeLocalRewriteAcceptedBinding>,
}

pub(super) struct RuntimeLocalRewriteAcceptedBinding {
    pub(super) identity: RuntimeProviderBindingIdentity,
    pub(super) previous_response_id: Option<String>,
    pub(super) turn_state: Option<String>,
    pub(super) session_id: Option<String>,
}

#[derive(Clone)]
pub(super) struct RuntimeLocalRewriteBindingContext {
    pub(super) previous_response_id: Option<String>,
    pub(super) turn_state: Option<String>,
    pub(super) session_id: Option<String>,
    pub(super) bound: Option<ResponseProfileBinding>,
}

impl RuntimeLocalRewriteBindingContext {
    pub(super) fn accepted_binding(
        &self,
        identity: RuntimeProviderBindingIdentity,
    ) -> RuntimeLocalRewriteAcceptedBinding {
        RuntimeLocalRewriteAcceptedBinding {
            identity,
            previous_response_id: self.previous_response_id.clone(),
            turn_state: self.turn_state.clone(),
            session_id: self.session_id.clone(),
        }
    }

    pub(super) fn candidate_allowed(
        &self,
        identity: Option<&RuntimeProviderBindingIdentity>,
    ) -> bool {
        match (&self.bound, identity) {
            (None, Some(_)) => true,
            (Some(binding), Some(identity)) => {
                binding.profile_name == RUNTIME_LOCAL_REWRITE_PROFILE
                    && binding.binding_identity.as_ref() == Some(identity)
            }
            _ => false,
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub(super) enum RuntimeLocalRewriteLiveBody {
    AsyncResponse(RuntimeLocalRewriteAsyncResponse),
    Prefetched(Box<RuntimeLocalRewriteContinuationReader>),
}

pub(super) enum RuntimeLocalRewriteNativeFirstEvent {
    Commit,
    Retry(ProviderErrorClass),
}

async fn runtime_local_rewrite_next_async_chunk(
    response: &mut reqwest::Response,
    pending: &mut Vec<u8>,
) -> std::result::Result<Option<bytes::Bytes>, reqwest::Error> {
    if !pending.is_empty() {
        return Ok(Some(bytes::Bytes::from(std::mem::take(pending))));
    }
    response.chunk().await
}

impl RuntimeLocalRewriteLiveBody {
    pub(super) fn into_reader(self) -> Box<dyn Read + Send> {
        match self {
            Self::AsyncResponse(response) => response.into_reader(),
            Self::Prefetched(reader) => reader,
        }
    }
}

pub(super) enum RuntimeLocalRewritePrefetchChunk {
    Data(Vec<u8>),
    End,
    Error(io::ErrorKind, String),
}

pub(super) struct RuntimeLocalRewriteSsePrefetch {
    receiver: Option<Receiver<RuntimeLocalRewritePrefetchChunk>>,
    backlog: VecDeque<RuntimeLocalRewritePrefetchChunk>,
    worker_abort: Option<tokio::task::AbortHandle>,
    cancelled: Arc<AtomicBool>,
    async_runtime: Arc<tokio::runtime::Runtime>,
    stream_idle_timeout_ms: u64,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

pub(super) struct RuntimeLocalRewriteContinuationReader {
    receiver: Receiver<RuntimeLocalRewritePrefetchChunk>,
    backlog: VecDeque<RuntimeLocalRewritePrefetchChunk>,
    pending: Cursor<Vec<u8>>,
    finished: bool,
    worker_abort: tokio::task::AbortHandle,
    cancelled: Arc<AtomicBool>,
    _async_runtime: Arc<tokio::runtime::Runtime>,
    stream_idle_timeout_ms: u64,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
}

pub(super) struct RuntimeLocalRewriteStreamingResponse {
    pub(super) status: u16,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Box<dyn std::io::Read + Send>,
    pub(super) profile_name: String,
    pub(super) accepted_binding_recorder: Option<RuntimeCopilotBindingRecorder>,
    pub(super) accepted_binding: Option<RuntimeLocalRewriteAcceptedBinding>,
}

impl RuntimeLocalRewriteLiveResponse {
    pub(super) fn new(response: RuntimeLocalRewriteAsyncResponse) -> Self {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        Self {
            status,
            headers,
            body: Some(RuntimeLocalRewriteLiveBody::AsyncResponse(response)),
            prefix: Vec::new(),
            upstream_eof: false,
            native_anthropic_messages: false,
            chat_compatible_request: None,
            accepted_binding_recorder: None,
            accepted_binding: None,
        }
    }

    pub(super) fn with_prefix(response: RuntimeLocalRewriteAsyncResponse, prefix: Vec<u8>) -> Self {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        Self {
            status,
            headers,
            body: Some(RuntimeLocalRewriteLiveBody::AsyncResponse(response)),
            prefix,
            upstream_eof: false,
            native_anthropic_messages: false,
            chat_compatible_request: None,
            accepted_binding_recorder: None,
            accepted_binding: None,
        }
    }

    pub(super) fn with_native_anthropic_messages(
        response: RuntimeLocalRewriteAsyncResponse,
    ) -> Self {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        Self {
            status,
            headers,
            body: Some(RuntimeLocalRewriteLiveBody::AsyncResponse(response)),
            prefix: Vec::new(),
            upstream_eof: false,
            native_anthropic_messages: true,
            chat_compatible_request: None,
            accepted_binding_recorder: None,
            accepted_binding: None,
        }
    }

    pub(super) fn with_chat_compatible_request(
        mut self,
        request: RuntimeDeepSeekPendingRequest,
    ) -> Self {
        self.chat_compatible_request = Some(request);
        self
    }

    pub(super) fn take_sse_prefetch(
        &mut self,
        permit: Option<tokio::sync::OwnedSemaphorePermit>,
    ) -> Result<RuntimeLocalRewriteSsePrefetch> {
        let body = self.body.take();
        match body {
            Some(RuntimeLocalRewriteLiveBody::AsyncResponse(response)) => {
                Ok(RuntimeLocalRewriteSsePrefetch::spawn(response, permit))
            }
            other => {
                self.body = other;
                Err(anyhow::anyhow!(
                    "runtime local rewrite SSE body was already handed off"
                ))
            }
        }
    }

    pub(super) fn set_sse_continuation(&mut self, prefetch: RuntimeLocalRewriteSsePrefetch) {
        self.body = Some(RuntimeLocalRewriteLiveBody::Prefetched(Box::new(
            prefetch.into_reader(),
        )));
    }
}

pub(super) fn runtime_local_rewrite_precommit_native_first_event(
    live: &mut RuntimeLocalRewriteLiveResponse,
    provider: RuntimeProviderBridgeKind,
    lookahead_timeout_ms: u64,
    prefetch_slots: &Arc<tokio::sync::Semaphore>,
) -> Result<RuntimeLocalRewriteNativeFirstEvent> {
    if !live.native_anthropic_messages
        || !(200..300).contains(&live.status)
        || !live.prefix.is_empty()
        || !live
            .headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"))
    {
        return Ok(RuntimeLocalRewriteNativeFirstEvent::Commit);
    }
    let Some(permit) = Arc::clone(prefetch_slots).try_acquire_owned().ok() else {
        return Ok(RuntimeLocalRewriteNativeFirstEvent::Commit);
    };
    let mut prefetch = live.take_sse_prefetch(Some(permit))?;
    let deadline = std::time::Instant::now() + Duration::from_millis(lookahead_timeout_ms);
    let mut prefix = Vec::new();
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut event_start = 0;

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() || prefix.len() >= crate::RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES {
            return Ok(runtime_local_rewrite_finish_native_prefetch(
                live,
                prefetch,
                prefix,
                false,
                RuntimeLocalRewriteNativeFirstEvent::Commit,
            ));
        }
        match prefetch.recv_timeout(remaining) {
            Ok(RuntimeLocalRewritePrefetchChunk::Data(chunk)) => {
                if let Some(decision) = runtime_local_rewrite_inspect_native_chunk(
                    &mut prefetch,
                    provider,
                    &mut prefix,
                    &mut line,
                    &mut data_lines,
                    &mut event_start,
                    chunk,
                ) {
                    return Ok(runtime_local_rewrite_finish_native_prefetch(
                        live, prefetch, prefix, false, decision,
                    ));
                }
            }
            Ok(RuntimeLocalRewritePrefetchChunk::End) => {
                prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::End);
                return Ok(runtime_local_rewrite_finish_native_prefetch(
                    live,
                    prefetch,
                    prefix,
                    true,
                    RuntimeLocalRewriteNativeFirstEvent::Commit,
                ));
            }
            Ok(RuntimeLocalRewritePrefetchChunk::Error(kind, message)) => {
                prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::Error(kind, message));
                return Ok(runtime_local_rewrite_finish_native_prefetch(
                    live,
                    prefetch,
                    prefix,
                    false,
                    RuntimeLocalRewriteNativeFirstEvent::Retry(ProviderErrorClass::Transient),
                ));
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Ok(runtime_local_rewrite_finish_native_prefetch(
                    live,
                    prefetch,
                    prefix,
                    false,
                    RuntimeLocalRewriteNativeFirstEvent::Commit,
                ));
            }
        }
    }
}

fn runtime_local_rewrite_inspect_native_chunk(
    prefetch: &mut RuntimeLocalRewriteSsePrefetch,
    provider: RuntimeProviderBridgeKind,
    prefix: &mut Vec<u8>,
    line: &mut Vec<u8>,
    data_lines: &mut Vec<String>,
    event_start: &mut usize,
    chunk: Vec<u8>,
) -> Option<RuntimeLocalRewriteNativeFirstEvent> {
    let inspect_len = chunk
        .len()
        .min(crate::RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES - prefix.len());
    let mut appended_until = 0;
    for (index, byte) in chunk[..inspect_len].iter().enumerate() {
        let mut event = None;
        runtime_proxy_crate::runtime_sse_consume_chunk(
            line,
            data_lines,
            std::slice::from_ref(byte),
            |parsed| event = Some(parsed),
        );
        let Some(event) = event else {
            continue;
        };
        prefix.extend_from_slice(&chunk[appended_until..=index]);
        appended_until = index + 1;
        let event_body = &prefix[*event_start..];
        if event.event_type.as_deref() == Some("ping") {
            *event_start = prefix.len();
            continue;
        }
        let decision = runtime_local_rewrite_native_first_event_error_class(provider, event_body)
            .map(RuntimeLocalRewriteNativeFirstEvent::Retry)
            .unwrap_or(RuntimeLocalRewriteNativeFirstEvent::Commit);
        if appended_until < chunk.len() {
            prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::Data(
                chunk[appended_until..].to_vec(),
            ));
        }
        return Some(decision);
    }
    prefix.extend_from_slice(&chunk[appended_until..inspect_len]);
    if inspect_len < chunk.len() {
        prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::Data(
            chunk[inspect_len..].to_vec(),
        ));
    }
    None
}

fn runtime_local_rewrite_finish_native_prefetch(
    live: &mut RuntimeLocalRewriteLiveResponse,
    prefetch: RuntimeLocalRewriteSsePrefetch,
    prefix: Vec<u8>,
    reached_upstream_end: bool,
    decision: RuntimeLocalRewriteNativeFirstEvent,
) -> RuntimeLocalRewriteNativeFirstEvent {
    live.prefix = prefix;
    if reached_upstream_end {
        live.upstream_eof = true;
    }
    live.set_sse_continuation(prefetch);
    decision
}

fn runtime_local_rewrite_native_first_event_error_class(
    _provider: RuntimeProviderBridgeKind,
    event: &[u8],
) -> Option<ProviderErrorClass> {
    let payload = event
        .split(|byte| *byte == b'\n')
        .filter_map(|line| line.strip_prefix(b"data:"))
        .filter_map(|line| std::str::from_utf8(line).ok())
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    let value = serde_json::from_str::<Value>(&payload).ok()?;
    let is_error =
        value.get("type").and_then(Value::as_str) == Some("error") || value.get("error").is_some();
    if !is_error {
        return None;
    }
    let code = value
        .pointer("/error/type")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/error/code").and_then(Value::as_str))
        .or_else(|| value.get("code").and_then(Value::as_str))
        .map(str::to_ascii_lowercase);
    match code.as_deref() {
        Some("rate_limit_error" | "rate_limit_exceeded" | "rate_limit_exceeded_error") => {
            Some(ProviderErrorClass::RateLimit)
        }
        Some("overloaded_error" | "server_is_overloaded") => Some(ProviderErrorClass::Transient),
        Some("not_found_error" | "model_not_supported") => Some(ProviderErrorClass::NotFound),
        Some("authentication_error") | Some("invalid_api_key") => Some(ProviderErrorClass::Auth),
        Some(
            "insufficient_quota" | "quota_exhausted" | "quota_exceeded" | "resource_exhausted",
        ) => Some(ProviderErrorClass::Quota),
        _ => None,
    }
}

impl RuntimeLocalRewriteSsePrefetch {
    pub(super) fn spawn(
        response: RuntimeLocalRewriteAsyncResponse,
        permit: Option<tokio::sync::OwnedSemaphorePermit>,
    ) -> Self {
        let RuntimeLocalRewriteAsyncResponse {
            response,
            async_runtime,
            stream_idle_timeout_ms,
            pending,
            reader,
            ..
        } = response;
        assert!(
            reader.is_none(),
            "runtime local rewrite stream reader should not be prefetched twice"
        );
        Self::spawn_parts(
            response.expect("runtime local rewrite upstream response should be present"),
            async_runtime,
            stream_idle_timeout_ms,
            pending,
            permit,
        )
    }

    pub(super) fn spawn_parts(
        response: reqwest::Response,
        async_runtime: Arc<tokio::runtime::Runtime>,
        stream_idle_timeout_ms: u64,
        mut pending: Vec<u8>,
        permit: Option<tokio::sync::OwnedSemaphorePermit>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(2);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = Arc::clone(&cancelled);
        let stream_idle_timeout = Duration::from_millis(stream_idle_timeout_ms.max(1));
        let mut upstream_response = response;
        let worker = async_runtime.spawn(async move {
            'stream: loop {
                if worker_cancelled.load(Ordering::Acquire) {
                    break;
                }
                let next = match tokio::time::timeout(
                    stream_idle_timeout,
                    runtime_local_rewrite_next_async_chunk(&mut upstream_response, &mut pending),
                )
                .await
                {
                    Ok(Ok(Some(chunk))) => {
                        for part in chunk.chunks(RUNTIME_LOCAL_REWRITE_STREAM_CHUNK_BYTES) {
                            if worker_cancelled.load(Ordering::Acquire)
                                || sender
                                    .send(RuntimeLocalRewritePrefetchChunk::Data(part.to_vec()))
                                    .await
                                    .is_err()
                            {
                                break 'stream;
                            }
                        }
                        continue;
                    }
                    Ok(Ok(None)) => RuntimeLocalRewritePrefetchChunk::End,
                    Err(_) => RuntimeLocalRewritePrefetchChunk::Error(
                        io::ErrorKind::TimedOut,
                        "runtime upstream stream idle timed out".to_string(),
                    ),
                    Ok(Err(error)) => RuntimeLocalRewritePrefetchChunk::Error(
                        crate::runtime_proxy::runtime_reqwest_error_kind(&error),
                        error.to_string(),
                    ),
                };
                let _ = sender.send(next).await;
                break;
            }
        });
        Self {
            receiver: Some(receiver),
            backlog: VecDeque::new(),
            worker_abort: Some(worker.abort_handle()),
            cancelled,
            async_runtime,
            stream_idle_timeout_ms,
            permit,
        }
    }

    pub(super) fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::result::Result<RuntimeLocalRewritePrefetchChunk, std::sync::mpsc::RecvTimeoutError>
    {
        if let Some(chunk) = self.backlog.pop_front() {
            return Ok(chunk);
        }
        let Some(receiver) = self.receiver.as_mut() else {
            return Err(std::sync::mpsc::RecvTimeoutError::Disconnected);
        };
        match self
            .async_runtime
            .block_on(async { tokio::time::timeout(timeout, receiver.recv()).await })
        {
            Ok(Some(chunk)) => Ok(chunk),
            Ok(None) => Err(std::sync::mpsc::RecvTimeoutError::Disconnected),
            Err(_) => Err(std::sync::mpsc::RecvTimeoutError::Timeout),
        }
    }

    pub(super) fn push_backlog(&mut self, chunk: RuntimeLocalRewritePrefetchChunk) {
        self.backlog.push_back(chunk);
    }

    pub(super) fn into_reader(mut self) -> RuntimeLocalRewriteContinuationReader {
        RuntimeLocalRewriteContinuationReader {
            receiver: self
                .receiver
                .take()
                .expect("runtime local rewrite prefetch receiver should be present"),
            backlog: std::mem::take(&mut self.backlog),
            pending: Cursor::new(Vec::new()),
            finished: false,
            worker_abort: self
                .worker_abort
                .take()
                .expect("runtime local rewrite prefetch abort handle should be present"),
            cancelled: Arc::clone(&self.cancelled),
            _async_runtime: Arc::clone(&self.async_runtime),
            stream_idle_timeout_ms: self.stream_idle_timeout_ms,
            permit: self.permit.take(),
        }
    }
}

impl Drop for RuntimeLocalRewriteSsePrefetch {
    fn drop(&mut self) {
        if let Some(worker_abort) = self.worker_abort.take() {
            self.cancelled.store(true, Ordering::Release);
            worker_abort.abort();
        }
    }
}

impl Read for RuntimeLocalRewriteContinuationReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.finished {
            return Ok(0);
        }
        loop {
            let read = self.pending.read(buffer)?;
            if read > 0 {
                return Ok(read);
            }
            let next = if let Some(chunk) = self.backlog.pop_front() {
                chunk
            } else {
                match self.recv_timeout(Duration::from_millis(self.stream_idle_timeout_ms.max(1))) {
                    Ok(chunk) => chunk,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        self.cancel();
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "runtime upstream stream idle timed out",
                        ));
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        self.cancel();
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "runtime upstream stream ended unexpectedly",
                        ));
                    }
                }
            };
            match next {
                RuntimeLocalRewritePrefetchChunk::Data(chunk) => {
                    self.pending = Cursor::new(chunk);
                }
                RuntimeLocalRewritePrefetchChunk::End => {
                    self.permit.take();
                    self.finished = true;
                    return Ok(0);
                }
                RuntimeLocalRewritePrefetchChunk::Error(kind, message) => {
                    self.permit.take();
                    self.finished = true;
                    return Err(io::Error::new(kind, message));
                }
            }
        }
    }
}

impl RuntimeLocalRewriteContinuationReader {
    fn recv_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::result::Result<RuntimeLocalRewritePrefetchChunk, std::sync::mpsc::RecvTimeoutError>
    {
        match self
            ._async_runtime
            .block_on(async { tokio::time::timeout(timeout, self.receiver.recv()).await })
        {
            Ok(Some(chunk)) => Ok(chunk),
            Ok(None) => Err(std::sync::mpsc::RecvTimeoutError::Disconnected),
            Err(_) => Err(std::sync::mpsc::RecvTimeoutError::Timeout),
        }
    }

    fn cancel(&mut self) {
        self.finished = true;
        self.cancelled.store(true, Ordering::Release);
        self.worker_abort.abort();
        self.permit.take();
    }
}

impl Drop for RuntimeLocalRewriteContinuationReader {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.worker_abort.abort();
    }
}

pub(super) fn runtime_local_rewrite_retryable_429_body(body: &[u8]) -> bool {
    runtime_proxy_crate::runtime_http_error_policy(
        429,
        body,
        runtime_proxy_crate::RuntimeHttpErrorPhase::PreCommit,
    )
    .action
        == runtime_proxy_crate::RuntimeHttpErrorAction::RotateProfile
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
            send_runtime_openai_upstream_request(
                request_id, request, shared, body, api_keys, endpoint,
            )
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

fn send_runtime_openai_upstream_request(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    body: Vec<u8>,
    api_keys: &[String],
    endpoint: ProviderEndpoint,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
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
    let binding = RuntimeLocalRewriteBindingContext {
        previous_response_id: runtime_local_rewrite_previous_response_id(&body),
        turn_state: runtime_proxy_crate::runtime_request_turn_state(request),
        session_id: runtime_proxy_crate::runtime_request_session_id(request),
        bound: None,
    };
    let binding = RuntimeLocalRewriteBindingContext {
        bound: runtime_local_rewrite_bound_binding(
            &shared.runtime_shared.runtime,
            binding.previous_response_id.as_deref(),
            binding.turn_state.as_deref(),
            binding.session_id.as_deref(),
        )?,
        ..binding
    };
    let (attempts, hard_binding) =
        runtime_local_rewrite_openai_attempts(shared, api_keys, binding.bound.as_ref())?;
    if !hard_binding && attempts.is_empty() && shared.provider_credential.is_none() {
        return runtime_local_rewrite_send_openai_unkeyed(
            request_id,
            request,
            shared,
            &upstream_url,
            body,
        );
    }
    if shared.provider_credential.is_some() {
        return runtime_local_rewrite_send_openai_projected(
            request_id,
            request,
            shared,
            &upstream_url,
            body,
            &binding,
        );
    }
    runtime_local_rewrite_send_openai_key_attempts(
        (request_id, request, shared),
        &upstream_url,
        body,
        attempts,
        hard_binding,
        &binding,
    )
}

fn runtime_local_rewrite_openai_attempts<'a>(
    shared: &RuntimeLocalRewriteProxyShared,
    api_keys: &'a [String],
    bound: Option<&ResponseProfileBinding>,
) -> Result<(Vec<(String, &'a str)>, bool)> {
    if let Some(bound) = bound {
        let Some(bound_identity) = bound.binding_identity.as_ref() else {
            return Err(anyhow::anyhow!(
                "OpenAI continuation binding has no exact key identity"
            ));
        };
        let attempts = runtime_local_rewrite_api_key_attempts(shared, api_keys)
            .into_iter()
            .filter(|(_, api_key)| {
                RuntimeProviderBindingIdentity::from_raw_key(
                    ProviderId::OpenAi,
                    api_key,
                    &shared.upstream_base_url,
                    None,
                )
                .is_some_and(|identity| identity == *bound_identity)
            })
            .collect::<Vec<_>>();
        if attempts.is_empty() {
            return Err(anyhow::anyhow!(
                "OpenAI continuation binding is unavailable or unauthorized"
            ));
        }
        return Ok((attempts, true));
    }
    if shared.provider_credential.is_some() {
        return Ok((Vec::new(), true));
    }
    let attempt_limit = shared
        .runtime_shared
        .runtime_config
        .tuning
        .precommit_attempt_limit
        .max(1);
    Ok((
        runtime_local_rewrite_api_key_attempts(shared, api_keys)
            .into_iter()
            .take(attempt_limit)
            .collect(),
        false,
    ))
}

fn runtime_local_rewrite_send_openai_unkeyed(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    upstream_url: &str,
    body: Vec<u8>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let response = send_runtime_local_rewrite_prepared_request(
        request_id,
        request,
        shared,
        upstream_url,
        body,
        RuntimeLocalRewritePreparedAuth::OpenAiResponses { api_key: None },
    )?;
    runtime_local_rewrite_openai_response(response, None, shared, None)
}

fn runtime_local_rewrite_send_openai_projected(
    request_id: u64,
    request: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    upstream_url: &str,
    body: Vec<u8>,
    binding: &RuntimeLocalRewriteBindingContext,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let binding_identity = shared
        .provider_credential
        .as_ref()
        .and_then(|credential| {
            runtime_provider_binding_identity_from_secret_ref(
                ProviderId::OpenAi,
                credential.reference(),
                &shared.upstream_base_url,
                Some(RUNTIME_LOCAL_REWRITE_PROFILE),
            )
        })
        .or_else(|| {
            RuntimeProviderBindingIdentity::from_profile(
                ProviderId::OpenAi,
                RUNTIME_LOCAL_REWRITE_PROFILE,
                &shared.upstream_base_url,
            )
        })
        .ok_or_else(|| anyhow::anyhow!("OpenAI projected binding is unavailable"))?;
    runtime_local_rewrite_validate_openai_projected_binding(binding, &binding_identity)?;
    let response = send_runtime_local_rewrite_prepared_request(
        request_id,
        request,
        shared,
        upstream_url,
        body,
        RuntimeLocalRewritePreparedAuth::OpenAiProjected,
    )?;
    runtime_local_rewrite_openai_response(response, Some(binding_identity), shared, Some(binding))
}

fn runtime_local_rewrite_validate_openai_projected_binding(
    binding: &RuntimeLocalRewriteBindingContext,
    identity: &RuntimeProviderBindingIdentity,
) -> Result<()> {
    if binding
        .bound
        .as_ref()
        .is_some_and(|binding| binding.profile_name != RUNTIME_LOCAL_REWRITE_PROFILE)
    {
        return Err(anyhow::anyhow!(
            "OpenAI continuation binding is unavailable or unauthorized"
        ));
    }
    if binding
        .bound
        .as_ref()
        .and_then(|binding| binding.binding_identity.as_ref())
        .is_some_and(|bound| bound != identity)
    {
        return Err(anyhow::anyhow!(
            "OpenAI continuation binding is conflicting"
        ));
    }
    Ok(())
}

fn runtime_local_rewrite_send_openai_key_attempts(
    request_context: (u64, &RuntimeProxyRequest, &RuntimeLocalRewriteProxyShared),
    upstream_url: &str,
    body: Vec<u8>,
    attempts: Vec<(String, &str)>,
    hard_binding: bool,
    binding: &RuntimeLocalRewriteBindingContext,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let (request_id, request, shared) = request_context;
    let attempt_count = attempts.len();
    for (attempt_index, (label, api_key)) in attempts.into_iter().enumerate() {
        let send_result = send_runtime_local_rewrite_prepared_request(
            request_id,
            request,
            shared,
            upstream_url,
            body.clone(),
            RuntimeLocalRewritePreparedAuth::OpenAiResponses {
                api_key: Some(api_key),
            },
        );
        let response = match send_result {
            Ok(response) => response,
            Err(_error) if !hard_binding && attempt_index + 1 < attempt_count => {
                runtime_proxy_log(
                    &shared.runtime_shared,
                    format!("openai_api_key_transport_retry request={request_id} attempt={label}"),
                );
                continue;
            }
            Err(error) => return Err(error),
        };
        if response.status().as_u16() >= 400 {
            let parts = runtime_local_rewrite_buffered_response_from_response(response)?;
            if runtime_local_rewrite_openai_error_can_retry(
                &parts,
                hard_binding,
                attempt_index,
                attempt_count,
            ) {
                continue;
            }
            return Ok(runtime_local_rewrite_buffered_result(parts));
        }
        let identity = RuntimeProviderBindingIdentity::from_raw_key(
            ProviderId::OpenAi,
            api_key,
            &shared.upstream_base_url,
            None,
        )
        .ok_or_else(|| anyhow::anyhow!("OpenAI accepted key identity is unavailable"))?;
        return runtime_local_rewrite_openai_response(
            response,
            Some(identity),
            shared,
            Some(binding),
        );
    }
    Err(anyhow::anyhow!("OpenAI API-key attempts were exhausted"))
}

fn runtime_local_rewrite_openai_error_can_retry(
    parts: &RuntimeHeapTrimmedBufferedResponseParts,
    hard_binding: bool,
    attempt_index: usize,
    attempt_count: usize,
) -> bool {
    let class = runtime_provider_error_class(
        RuntimeProviderBridgeKind::OpenAiResponses,
        parts.status,
        &parts.body,
    );
    !hard_binding
        && (parts.status != 429 || runtime_local_rewrite_retryable_429_body(&parts.body))
        && matches!(
            class,
            ProviderErrorClass::Auth
                | ProviderErrorClass::RateLimit
                | ProviderErrorClass::Quota
                | ProviderErrorClass::Transient
        )
        && attempt_index + 1 < attempt_count
}

fn runtime_local_rewrite_openai_response(
    response: RuntimeLocalRewriteAsyncResponse,
    identity: Option<RuntimeProviderBindingIdentity>,
    shared: &RuntimeLocalRewriteProxyShared,
    binding: Option<&RuntimeLocalRewriteBindingContext>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    if response.status().as_u16() >= 400 {
        return Ok(runtime_local_rewrite_buffered_result(
            runtime_local_rewrite_buffered_response_from_response(response)?,
        ));
    }
    let mut live = RuntimeLocalRewriteLiveResponse::new(response);
    if let Some(identity) = identity {
        live.accepted_binding_recorder = Some(runtime_local_rewrite_binding_recorder(
            shared,
            identity.clone(),
        ));
        live.accepted_binding = binding.map(|binding| binding.accepted_binding(identity));
    }
    Ok(runtime_local_rewrite_live_result(live))
}

fn runtime_local_rewrite_buffered_result(
    parts: RuntimeHeapTrimmedBufferedResponseParts,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Buffered(parts),
        gemini_context: None,
        copilot_context: None,
    }
}

fn runtime_local_rewrite_live_result(
    live: RuntimeLocalRewriteLiveResponse,
) -> RuntimeLocalRewriteUpstreamResult {
    RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Live(live),
        gemini_context: None,
        copilot_context: None,
    }
}

pub(super) fn runtime_local_rewrite_previous_response_id(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("previous_response_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

pub(super) fn runtime_local_rewrite_binding_context(
    shared: &RuntimeLocalRewriteProxyShared,
    request: &RuntimeProxyRequest,
) -> Result<RuntimeLocalRewriteBindingContext> {
    let previous_response_id = runtime_local_rewrite_previous_response_id(&request.body);
    let turn_state = runtime_proxy_crate::runtime_request_turn_state(request);
    let session_id = runtime_proxy_crate::runtime_request_session_id(request);
    let bound = runtime_local_rewrite_bound_binding(
        &shared.runtime_shared.runtime,
        previous_response_id.as_deref(),
        turn_state.as_deref(),
        session_id.as_deref(),
    )?;
    Ok(RuntimeLocalRewriteBindingContext {
        previous_response_id,
        turn_state,
        session_id,
        bound,
    })
}

pub(super) fn runtime_local_rewrite_raw_binding_identity(
    shared: &RuntimeLocalRewriteProxyShared,
    provider: ProviderId,
    credential: Option<&str>,
    endpoint: &str,
    profile: Option<&str>,
) -> Option<RuntimeProviderBindingIdentity> {
    credential
        .and_then(|credential| {
            RuntimeProviderBindingIdentity::from_raw_key(provider, credential, endpoint, profile)
        })
        .or_else(|| {
            shared.provider_credential.as_ref().and_then(|credential| {
                runtime_provider_binding_identity_from_secret_ref(
                    provider,
                    credential.reference(),
                    endpoint,
                    profile,
                )
            })
        })
        .or_else(|| {
            profile.and_then(|profile| {
                RuntimeProviderBindingIdentity::from_profile(provider, profile, endpoint)
            })
        })
}

pub(super) fn runtime_local_rewrite_continuation_is_bound(
    shared: &RuntimeLocalRewriteProxyShared,
    request: &RuntimeProxyRequest,
) -> Result<bool> {
    Ok(runtime_local_rewrite_request_bound_binding(shared, request)?.is_some())
}

pub(super) fn runtime_local_rewrite_request_bound_binding(
    shared: &RuntimeLocalRewriteProxyShared,
    request: &RuntimeProxyRequest,
) -> Result<Option<ResponseProfileBinding>> {
    let previous_response_id = runtime_local_rewrite_previous_response_id(&request.body);
    let turn_state = runtime_proxy_crate::runtime_request_turn_state(request);
    let session_id = runtime_proxy_crate::runtime_request_session_id(request);
    runtime_local_rewrite_bound_binding(
        &shared.runtime_shared.runtime,
        previous_response_id.as_deref(),
        turn_state.as_deref(),
        session_id.as_deref(),
    )
}

pub(super) fn runtime_local_rewrite_bound_binding(
    runtime_state: &Arc<std::sync::Mutex<crate::RuntimeRotationState>>,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<Option<ResponseProfileBinding>> {
    let Some(identity) = prodex_runtime_state::RuntimeHardBindingIdentity::new(
        previous_response_id,
        turn_state,
        session_id,
    ) else {
        let supplied = [previous_response_id, turn_state, session_id]
            .into_iter()
            .flatten()
            .any(|value| !value.trim().is_empty());
        return if supplied {
            Err(anyhow::anyhow!(
                "local rewrite continuation identity is invalid"
            ))
        } else {
            Ok(None)
        };
    };
    let runtime = runtime_state
        .lock()
        .map_err(|_| anyhow::anyhow!("runtime auto-rotate state is poisoned"))?;
    let mut binding = None;
    let mut absorb = |candidate: Option<&ResponseProfileBinding>| -> Result<()> {
        let Some(candidate) = candidate else {
            return Ok(());
        };
        if !prodex_runtime_state::runtime_identity_component_is_valid(&candidate.profile_name)
            || prodex_state::is_hard_binding_conflict_profile(&candidate.profile_name)
        {
            return Err(anyhow::anyhow!(
                "local rewrite continuation binding is conflicting"
            ));
        }
        let merged = binding
            .as_ref()
            .map(|existing| prodex_state::merge_response_profile_binding(existing, candidate))
            .unwrap_or_else(|| candidate.clone());
        if prodex_state::is_hard_binding_conflict_profile(&merged.profile_name) {
            return Err(anyhow::anyhow!(
                "local rewrite continuation binding is conflicting"
            ));
        }
        binding = Some(merged);
        Ok(())
    };
    absorb(
        identity
            .response_id
            .as_deref()
            .and_then(|key| runtime.state.response_profile_bindings.get(key)),
    )?;
    absorb(
        identity
            .turn_state
            .as_deref()
            .and_then(|key| runtime.turn_state_bindings.get(key)),
    )?;
    if let (Some(response_id), Some(turn_state)) = (
        identity.response_id.as_deref(),
        identity.turn_state.as_deref(),
    ) {
        absorb(runtime.state.response_profile_bindings.get(
            &prodex_runtime_state::runtime_response_turn_state_lineage_key(response_id, turn_state),
        ))?;
    }
    if let Some(turn_state) = identity.turn_state.as_deref() {
        absorb(
            runtime
                .turn_state_bindings
                .get(&prodex_runtime_state::runtime_compact_turn_state_lineage_key(turn_state)),
        )?;
    }
    if let Some(session_id) = identity.session_id.as_deref() {
        absorb(runtime.session_id_bindings.get(session_id))?;
        absorb(runtime.session_id_bindings.get(
            &prodex_runtime_state::runtime_compact_session_lineage_key(session_id),
        ))?;
        absorb(runtime.state.session_profile_bindings.get(session_id))?;
    }
    Ok(binding)
}

pub(super) fn runtime_local_rewrite_remember_accepted_binding(
    shared: &RuntimeLocalRewriteProxyShared,
    binding_identity: &RuntimeProviderBindingIdentity,
    previous_response_id: Option<&str>,
    turn_state: Option<&str>,
    session_id: Option<&str>,
) -> Result<()> {
    let response_ids = previous_response_id
        .map(str::to_string)
        .into_iter()
        .collect::<Vec<_>>();
    crate::runtime_proxy::remember_runtime_external_binding_identity(
        &shared.runtime_shared,
        RUNTIME_LOCAL_REWRITE_PROFILE,
        binding_identity,
        &response_ids,
        turn_state,
        session_id,
    )
}

pub(super) fn runtime_local_rewrite_attach_accepted_binding(
    shared: &RuntimeLocalRewriteProxyShared,
    response: &mut RuntimeLocalRewriteLiveResponse,
    binding: &RuntimeLocalRewriteBindingContext,
    binding_identity: RuntimeProviderBindingIdentity,
) {
    response.accepted_binding_recorder = Some(runtime_local_rewrite_binding_recorder(
        shared,
        binding_identity.clone(),
    ));
    response.accepted_binding = Some(binding.accepted_binding(binding_identity));
}

pub(super) fn runtime_local_rewrite_binding_recorder(
    shared: &RuntimeLocalRewriteProxyShared,
    binding_identity: RuntimeProviderBindingIdentity,
) -> RuntimeCopilotBindingRecorder {
    let runtime_shared = shared.runtime_shared.clone();
    Arc::new(move |response_id| {
        let response_ids = vec![response_id];
        let _ = crate::runtime_proxy::remember_runtime_external_binding_identity(
            &runtime_shared,
            RUNTIME_LOCAL_REWRITE_PROFILE,
            &binding_identity,
            &response_ids,
            None,
            None,
        );
    })
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
    use super::super::provider_bridge::RuntimeProviderBridgeKind;
    use super::{
        RuntimeLocalRewriteAsyncResponse, RuntimeLocalRewriteBindingContext,
        RuntimeLocalRewriteContinuationReader, RuntimeLocalRewriteLiveResponse,
        RuntimeLocalRewriteNativeFirstEvent, RuntimeLocalRewriteSsePrefetch,
        runtime_local_rewrite_precommit_native_first_event,
        runtime_local_rewrite_retryable_429_body,
    };
    use prodex_provider_core::{
        ProviderEndpoint, ProviderErrorClass, ProviderId, ProviderTransformInput,
        ProviderTransformLoss, RuntimeProviderBindingIdentity, provider_core_lossless_body,
        provider_translator,
    };
    use prodex_state::ResponseProfileBinding;
    use std::collections::VecDeque;
    use std::io::{self, Cursor, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    #[test]
    fn raw_provider_binding_locks_response_turn_session_to_exact_identity() {
        let endpoint = "https://provider.example.com/v1";
        for provider in [
            ProviderId::Anthropic,
            ProviderId::DeepSeek,
            ProviderId::Gemini,
            ProviderId::Kiro,
        ] {
            let identity = |credential: &str, profile: &str, endpoint: &str| {
                if provider == ProviderId::Kiro {
                    RuntimeProviderBindingIdentity::from_profile(provider, profile, endpoint)
                } else {
                    RuntimeProviderBindingIdentity::from_raw_key(
                        provider,
                        credential,
                        endpoint,
                        Some(profile),
                    )
                }
                .expect("synthetic provider identity should be valid")
            };
            let expected = identity("key-a", "profile-a", endpoint);
            let binding = RuntimeLocalRewriteBindingContext {
                previous_response_id: Some("resp_example".to_string()),
                turn_state: Some("turn_example".to_string()),
                session_id: Some("session_example".to_string()),
                bound: Some(ResponseProfileBinding {
                    profile_name: super::RUNTIME_LOCAL_REWRITE_PROFILE.to_string(),
                    bound_at: 1,
                    binding_identity: Some(expected.clone()),
                }),
            };
            let accepted = binding.accepted_binding(expected.clone());
            assert_eq!(
                accepted.previous_response_id.as_deref(),
                Some("resp_example")
            );
            assert_eq!(accepted.turn_state.as_deref(), Some("turn_example"));
            assert_eq!(accepted.session_id.as_deref(), Some("session_example"));
            assert!(binding.candidate_allowed(Some(&expected)));
            if provider != ProviderId::Kiro {
                assert!(!binding.candidate_allowed(Some(&identity(
                    "key-b",
                    "profile-a",
                    endpoint,
                ))));
            }
            assert!(!binding.candidate_allowed(Some(&identity("key-a", "profile-b", endpoint,))));
            assert!(!binding.candidate_allowed(Some(&identity(
                "key-a",
                "profile-a",
                "https://other.example.com/v1",
            ))));

            let fresh = RuntimeLocalRewriteBindingContext {
                bound: None,
                ..binding
            };
            assert!(fresh.candidate_allowed(Some(&expected)));
            assert!(fresh.candidate_allowed(Some(&identity("key-b", "profile-b", endpoint,))));
        }
    }

    fn test_async_runtime() -> Arc<tokio::runtime::Runtime> {
        Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("SSE test runtime should build"),
        )
    }

    fn mock_async_sse_response(
        chunks: Vec<(Duration, Vec<u8>)>,
        stream_idle_timeout_ms: u64,
        hold_open: bool,
    ) -> (
        super::RuntimeLocalRewriteAsyncResponse,
        Arc<tokio::runtime::Runtime>,
        JoinHandle<()>,
        Arc<AtomicBool>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock upstream should bind");
        let address = listener
            .local_addr()
            .expect("mock upstream address should be available");
        let closed = Arc::new(AtomicBool::new(false));
        let server_closed = Arc::clone(&closed);
        let server = thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
            read_mock_request(&mut stream);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            );
            let _ = stream.flush();
            for (delay, chunk) in chunks {
                thread::sleep(delay);
                let _ = stream.write_all(format!("{:x}\r\n", chunk.len()).as_bytes());
                let _ = stream.write_all(&chunk);
                let _ = stream.write_all(b"\r\n");
                let _ = stream.flush();
            }
            if hold_open {
                let mut byte = [0_u8; 1];
                let deadline = Instant::now() + Duration::from_secs(2);
                while Instant::now() < deadline {
                    match stream.read(&mut byte) {
                        Ok(0) => {
                            server_closed.store(true, Ordering::Release);
                            return;
                        }
                        Ok(_) => {}
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) => {}
                        Err(_) => return,
                    }
                }
            } else {
                let _ = stream.write_all(b"0\r\n\r\n");
                let _ = stream.flush();
                server_closed.store(true, Ordering::Release);
            }
        });
        let runtime = test_async_runtime();
        let response = runtime
            .block_on(
                reqwest::Client::new()
                    .get(format!("http://{address}/v1/responses"))
                    .send(),
            )
            .expect("mock upstream response should arrive");
        (
            super::RuntimeLocalRewriteAsyncResponse::new(
                response,
                Arc::clone(&runtime),
                stream_idle_timeout_ms,
            ),
            runtime,
            server,
            closed,
        )
    }

    fn mock_truncated_async_sse_response() -> (
        super::RuntimeLocalRewriteAsyncResponse,
        Arc<tokio::runtime::Runtime>,
        JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock upstream should bind");
        let address = listener
            .local_addr()
            .expect("mock upstream address should be available");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("mock upstream should accept");
            read_mock_request(&mut stream);
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 1\r\nConnection: close\r\n\r\n",
                )
                .expect("mock upstream headers should write");
            stream.flush().expect("mock upstream should flush");
        });
        let runtime = test_async_runtime();
        let response = runtime
            .block_on(
                reqwest::Client::new()
                    .get(format!("http://{address}/v1/responses"))
                    .send(),
            )
            .expect("mock upstream response should arrive");
        (
            super::RuntimeLocalRewriteAsyncResponse::new(response, Arc::clone(&runtime), 100),
            runtime,
            server,
        )
    }

    fn read_mock_request(stream: &mut TcpStream) {
        let mut request = Vec::new();
        let mut chunk = [0_u8; 256];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => request.extend_from_slice(&chunk[..read]),
            }
        }
    }

    fn read_prefetch_body(prefetch: super::RuntimeLocalRewriteSsePrefetch) -> io::Result<Vec<u8>> {
        let mut reader = prefetch.into_reader();
        let mut body = Vec::new();
        reader.read_to_end(&mut body)?;
        Ok(body)
    }

    fn read_live_body(mut live: RuntimeLocalRewriteLiveResponse) -> io::Result<Vec<u8>> {
        let mut body = live.prefix;
        live.body
            .take()
            .expect("native SSE body should remain available")
            .into_reader()
            .read_to_end(&mut body)?;
        Ok(body)
    }

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

    #[test]
    fn local_prefetch_drop_releases_its_slot_immediately() {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("prefetch test runtime should build"),
        );
        let worker = runtime.spawn(async {});
        let worker_abort = worker.abort_handle();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = Arc::clone(&semaphore)
            .try_acquire_owned()
            .expect("prefetch slot should be available");
        let cancelled = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = tokio::sync::mpsc::channel(2);
        drop(sender);

        let reader = RuntimeLocalRewriteContinuationReader {
            receiver,
            backlog: VecDeque::new(),
            pending: Cursor::new(Vec::new()),
            finished: false,
            worker_abort,
            cancelled: Arc::clone(&cancelled),
            _async_runtime: Arc::clone(&runtime),
            stream_idle_timeout_ms: 100,
            permit: Some(permit),
        };
        assert_eq!(semaphore.available_permits(), 0);
        drop(reader);
        assert!(cancelled.load(Ordering::Acquire));
        assert_eq!(semaphore.available_permits(), 1);

        runtime.block_on(async {
            let _ = worker.await;
        });
    }

    #[test]
    fn local_prefetch_channel_close_releases_its_slot() {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("prefetch test runtime should build"),
        );
        let worker = runtime.spawn(async {});
        let worker_abort = worker.abort_handle();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = Arc::clone(&semaphore)
            .try_acquire_owned()
            .expect("prefetch slot should be available");
        let cancelled = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = tokio::sync::mpsc::channel(2);
        drop(sender);
        let mut reader = RuntimeLocalRewriteContinuationReader {
            receiver,
            backlog: VecDeque::new(),
            pending: Cursor::new(Vec::new()),
            finished: false,
            worker_abort,
            cancelled: Arc::clone(&cancelled),
            _async_runtime: Arc::clone(&runtime),
            stream_idle_timeout_ms: 100,
            permit: Some(permit),
        };
        let mut buffer = [0_u8; 1];

        let error = reader
            .read(&mut buffer)
            .expect_err("closed channel should report unexpected EOF");
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
        assert!(reader.finished);
        assert!(cancelled.load(Ordering::Acquire));
        assert_eq!(semaphore.available_permits(), 1);

        runtime.block_on(async {
            let _ = worker.await;
        });
    }

    #[test]
    fn local_prefetch_open_channel_times_out_and_releases_its_slot() {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("prefetch test runtime should build"),
        );
        let worker = runtime.spawn(async {});
        let worker_abort = worker.abort_handle();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = Arc::clone(&semaphore)
            .try_acquire_owned()
            .expect("prefetch slot should be available");
        let cancelled = Arc::new(AtomicBool::new(false));
        let (_sender, receiver) = tokio::sync::mpsc::channel(2);
        let mut reader = RuntimeLocalRewriteContinuationReader {
            receiver,
            backlog: VecDeque::new(),
            pending: Cursor::new(Vec::new()),
            finished: false,
            worker_abort,
            cancelled: Arc::clone(&cancelled),
            _async_runtime: Arc::clone(&runtime),
            stream_idle_timeout_ms: 10,
            permit: Some(permit),
        };
        let mut buffer = [0_u8; 1];

        let error = reader
            .read(&mut buffer)
            .expect_err("open channel should honor the stream idle timeout");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(reader.finished);
        assert!(cancelled.load(Ordering::Acquire));
        assert_eq!(semaphore.available_permits(), 1);

        runtime.block_on(async {
            let _ = worker.await;
        });
    }

    #[test]
    fn generic_429_body_is_not_retryable() {
        assert!(!runtime_local_rewrite_retryable_429_body(
            b"too many requests"
        ));
        assert!(runtime_local_rewrite_retryable_429_body(
            br#"{"error":{"code":"insufficient_quota"}}"#
        ));
    }

    #[test]
    fn native_first_event_retries_only_explicit_error_codes() {
        for (body, expected) in [
            (
                br#"data: {"type":"error","error":{"type":"rate_limit_error"}}

"#
                .to_vec(),
                RuntimeLocalRewriteNativeFirstEvent::Retry(ProviderErrorClass::RateLimit),
            ),
            (
                br#"data: {"type":"error","error":{"type":"overloaded_error"}}

"#
                .to_vec(),
                RuntimeLocalRewriteNativeFirstEvent::Retry(ProviderErrorClass::Transient),
            ),
            (
                br#"data: {"type":"error","error":{"message":"too many requests"}}

"#
                .to_vec(),
                RuntimeLocalRewriteNativeFirstEvent::Commit,
            ),
        ] {
            let (response, runtime, server, _) =
                mock_async_sse_response(vec![(Duration::ZERO, body)], 100, false);
            let mut live =
                RuntimeLocalRewriteLiveResponse::with_native_anthropic_messages(response);
            let outcome = runtime_local_rewrite_precommit_native_first_event(
                &mut live,
                RuntimeProviderBridgeKind::Anthropic,
                50,
                &Arc::new(tokio::sync::Semaphore::new(1)),
            )
            .expect("native first-event lookahead should succeed");
            match (outcome, expected) {
                (
                    RuntimeLocalRewriteNativeFirstEvent::Retry(actual),
                    RuntimeLocalRewriteNativeFirstEvent::Retry(expected),
                ) => assert_eq!(actual, expected),
                (
                    RuntimeLocalRewriteNativeFirstEvent::Commit,
                    RuntimeLocalRewriteNativeFirstEvent::Commit,
                ) => {}
                _ => panic!("native first-event classification changed"),
            }
            drop(live);
            runtime.block_on(async { tokio::task::yield_now().await });
            server.join().expect("mock upstream should finish");
        }
    }

    #[test]
    fn native_first_event_transport_failure_is_precommit_transient() {
        let (response, runtime, server) = mock_truncated_async_sse_response();
        let mut live = RuntimeLocalRewriteLiveResponse::with_native_anthropic_messages(response);

        let outcome = runtime_local_rewrite_precommit_native_first_event(
            &mut live,
            RuntimeProviderBridgeKind::DeepSeek,
            50,
            &Arc::new(tokio::sync::Semaphore::new(1)),
        )
        .expect("native first-event lookahead should classify transport failure");

        assert!(matches!(
            outcome,
            RuntimeLocalRewriteNativeFirstEvent::Retry(ProviderErrorClass::Transient)
        ));
        assert!(read_live_body(live).is_err());
        runtime.block_on(async { tokio::task::yield_now().await });
        server.join().expect("mock upstream should finish");
    }

    #[test]
    fn native_first_event_commits_model_output_before_later_error() {
        let body = br#"data: {"type":"message_start","message":{"id":"msg_example"}}

data: {"type":"error","error":{"type":"overloaded_error"}}

"#
        .to_vec();
        let expected = body.clone();
        let (response, runtime, server, _) =
            mock_async_sse_response(vec![(Duration::ZERO, body)], 100, false);
        let mut live = RuntimeLocalRewriteLiveResponse::with_native_anthropic_messages(response);

        let outcome = runtime_local_rewrite_precommit_native_first_event(
            &mut live,
            RuntimeProviderBridgeKind::Anthropic,
            50,
            &Arc::new(tokio::sync::Semaphore::new(1)),
        )
        .expect("native first-event lookahead should succeed");

        assert!(matches!(
            outcome,
            RuntimeLocalRewriteNativeFirstEvent::Commit
        ));
        assert_eq!(read_live_body(live).unwrap(), expected);
        runtime.block_on(async { tokio::task::yield_now().await });
        server.join().expect("mock upstream should finish");
    }

    #[test]
    fn native_first_event_preserves_exact_bytes_when_retry_is_returned() {
        let body = br#"data: {"type":"error","error":{"type":"rate_limit_error"}}

data: {"type":"message_start","message":{"id":"msg_example"}}

"#
        .to_vec();
        let expected = body.clone();
        let (response, runtime, server, _) =
            mock_async_sse_response(vec![(Duration::ZERO, body)], 100, false);
        let mut live = RuntimeLocalRewriteLiveResponse::with_native_anthropic_messages(response);
        let outcome = runtime_local_rewrite_precommit_native_first_event(
            &mut live,
            RuntimeProviderBridgeKind::Anthropic,
            50,
            &Arc::new(tokio::sync::Semaphore::new(1)),
        )
        .expect("native first-event lookahead should succeed");
        assert!(matches!(
            outcome,
            RuntimeLocalRewriteNativeFirstEvent::Retry(ProviderErrorClass::RateLimit)
        ));
        assert_eq!(
            read_live_body(live).expect("native body should remain readable"),
            expected
        );
        runtime.block_on(async { tokio::task::yield_now().await });
        server.join().expect("mock upstream should finish");
    }

    #[test]
    fn async_prefetch_accepts_gaps_below_idle_timeout() {
        let (response, runtime, server, _) = mock_async_sse_response(
            vec![
                (Duration::ZERO, b"first".to_vec()),
                (Duration::from_millis(40), b"second".to_vec()),
            ],
            100,
            false,
        );
        let body = read_prefetch_body(super::RuntimeLocalRewriteSsePrefetch::spawn(response, None))
            .expect("sub-idle gaps should keep stream readable");
        assert_eq!(body, b"firstsecond");
        runtime.block_on(async { tokio::task::yield_now().await });
        server.join().expect("mock upstream should finish");
    }

    #[test]
    fn async_prefetch_keeps_owned_runtime_alive_for_fixed_length_body() {
        let body = b"data: {\"error\":{\"code\":\"rate_limit_exceeded\"}}\n\n".to_vec();
        let server = tiny_http::Server::http("127.0.0.1:0").expect("SSE test server should bind");
        let address = server
            .server_addr()
            .to_ip()
            .expect("SSE test server should expose an IP address");
        let expected = body.clone();
        let server_thread = thread::spawn(move || {
            let request = server
                .recv()
                .expect("SSE test server should receive a request");
            request
                .respond(tiny_http::Response::new(
                    tiny_http::StatusCode(200),
                    vec![
                        tiny_http::Header::from_bytes("content-type", "text/event-stream")
                            .expect("SSE content type header"),
                    ],
                    Cursor::new(body.clone()),
                    Some(body.len()),
                    None,
                ))
                .expect("SSE test server should respond");
        });
        let runtime = test_async_runtime();
        let response = runtime
            .block_on(
                reqwest::Client::new()
                    .get(format!("http://{address}"))
                    .send(),
            )
            .expect("SSE test client should receive a response");
        server_thread.join().expect("SSE test server should finish");
        let response = RuntimeLocalRewriteAsyncResponse::new(
            response,
            runtime,
            crate::RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
        );
        let body = read_prefetch_body(RuntimeLocalRewriteSsePrefetch::spawn(response, None))
            .expect("fixed-length SSE should remain readable");
        assert_eq!(body, expected);
    }

    #[test]
    fn async_prefetch_times_out_gaps_above_idle_timeout() {
        let (response, runtime, server, _) = mock_async_sse_response(
            vec![
                (Duration::ZERO, b"first".to_vec()),
                (Duration::from_millis(80), b"second".to_vec()),
            ],
            15,
            false,
        );
        let mut reader = super::RuntimeLocalRewriteSsePrefetch::spawn(response, None).into_reader();
        let mut body = Vec::new();
        let error = reader
            .read_to_end(&mut body)
            .expect_err("over-idle gap should be reported");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert_eq!(body, b"first");
        runtime.block_on(async { tokio::task::yield_now().await });
        server.join().expect("mock upstream should finish");
    }

    #[test]
    fn async_prefetch_resets_idle_timeout_after_each_chunk() {
        let (response, runtime, server, _) = mock_async_sse_response(
            vec![
                (Duration::ZERO, b"a".to_vec()),
                (Duration::from_millis(8), b"b".to_vec()),
                (Duration::from_millis(8), b"c".to_vec()),
            ],
            25,
            false,
        );
        let body = read_prefetch_body(super::RuntimeLocalRewriteSsePrefetch::spawn(response, None))
            .expect("each chunk should reset idle timeout");
        assert_eq!(body, b"abc");
        runtime.block_on(async { tokio::task::yield_now().await });
        server.join().expect("mock upstream should finish");
    }

    #[test]
    fn async_prefetch_is_bounded_for_slow_consumers() {
        let chunks = (0..8)
            .map(|index| (Duration::ZERO, vec![b'a' + index; 1024]))
            .collect();
        let (response, runtime, server, _) = mock_async_sse_response(chunks, 100, false);
        let prefetch = super::RuntimeLocalRewriteSsePrefetch::spawn(response, None);
        runtime.block_on(async { tokio::time::sleep(Duration::from_millis(20)).await });
        let receiver = prefetch
            .receiver
            .as_ref()
            .expect("bounded bridge receiver should exist");
        assert_eq!(receiver.capacity() + receiver.len(), 2);
        thread::sleep(Duration::from_millis(10));
        let body = read_prefetch_body(prefetch).expect("slow consumer should eventually drain");
        assert_eq!(body.len(), 8 * 1024);
        server.join().expect("mock upstream should finish");
    }

    #[test]
    fn async_prefetch_splits_large_upstream_chunks_before_queueing() {
        let expected = vec![b'x'; super::RUNTIME_LOCAL_REWRITE_STREAM_CHUNK_BYTES * 3 + 7];
        let (mut response, runtime, server, _) = mock_async_sse_response(Vec::new(), 100, false);
        response.pending = expected.clone();
        let mut prefetch = super::RuntimeLocalRewriteSsePrefetch::spawn(response, None);
        let mut actual = Vec::new();
        loop {
            match prefetch
                .recv_timeout(Duration::from_secs(1))
                .expect("bounded upstream chunk should arrive")
            {
                super::RuntimeLocalRewritePrefetchChunk::Data(chunk) => {
                    assert!(chunk.len() <= super::RUNTIME_LOCAL_REWRITE_STREAM_CHUNK_BYTES);
                    actual.extend_from_slice(&chunk);
                }
                super::RuntimeLocalRewritePrefetchChunk::End => break,
                super::RuntimeLocalRewritePrefetchChunk::Error(_, message) => {
                    panic!("unexpected bounded stream error: {message}")
                }
            }
        }
        assert_eq!(actual, expected);
        runtime.block_on(async { tokio::task::yield_now().await });
        server.join().expect("mock upstream should finish");
    }

    #[test]
    fn async_prefetch_drop_aborts_pump_and_releases_permit() {
        let (response, runtime, server, closed) =
            mock_async_sse_response(vec![(Duration::ZERO, b"first".to_vec())], 100, true);
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = Arc::clone(&semaphore)
            .try_acquire_owned()
            .expect("prefetch slot should be available");
        let prefetch = super::RuntimeLocalRewriteSsePrefetch::spawn(response, Some(permit));
        let cancelled = Arc::clone(&prefetch.cancelled);
        assert_eq!(semaphore.available_permits(), 0);
        drop(prefetch);
        assert!(cancelled.load(Ordering::Acquire));
        assert_eq!(semaphore.available_permits(), 1);
        runtime.block_on(async { tokio::time::sleep(Duration::from_millis(25)).await });
        server
            .join()
            .expect("mock upstream should observe cancellation");
        assert!(closed.load(Ordering::Acquire));
    }
}
