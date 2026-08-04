use super::super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewritePrefetchChunk,
    RuntimeLocalRewriteSsePrefetch,
};
use super::super::{
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, RuntimeProviderBridgeKind, runtime_provider_error_class,
};
use crate::runtime_proxy::{
    bump_runtime_profile_health_score, commit_runtime_proxy_profile_selection_with_policy,
    note_runtime_profile_transport_failure,
};
use crate::{
    RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY, RuntimeHeapTrimmedBufferedResponseParts,
    RuntimeRouteKind,
};
use prodex_provider_core::ProviderErrorClass;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(super) fn runtime_local_rewrite_record_provider_health(
    shared: &RuntimeLocalRewriteProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    result: &anyhow::Result<RuntimeLocalRewriteUpstreamResult>,
    fallback_class: Option<ProviderErrorClass>,
) {
    match result {
        Err(error) => note_runtime_profile_transport_failure(
            &shared.runtime_shared,
            profile_name,
            route_kind,
            "governed_provider_dispatch",
            error,
        ),
        Ok(response)
            if fallback_class == Some(ProviderErrorClass::Transient)
                || response.status() == 503 =>
        {
            let _ = bump_runtime_profile_health_score(
                &shared.runtime_shared,
                profile_name,
                route_kind,
                RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                "governed_provider_overload",
            );
        }
        Ok(response) if (200..400).contains(&response.status()) && fallback_class.is_none() => {
            let _ = commit_runtime_proxy_profile_selection_with_policy(
                &shared.runtime_shared,
                profile_name,
                route_kind,
                false,
            );
        }
        Ok(_) => {}
    }
}

pub(super) fn runtime_local_rewrite_record_provider_metric(
    provider: RuntimeProviderBridgeKind,
    result: &anyhow::Result<RuntimeLocalRewriteUpstreamResult>,
    fallback_class: Option<ProviderErrorClass>,
    duration: Duration,
) {
    let provider = match provider {
        RuntimeProviderBridgeKind::OpenAiResponses => prodex_observability::ProviderKind::OpenAi,
        RuntimeProviderBridgeKind::Anthropic => prodex_observability::ProviderKind::Anthropic,
        RuntimeProviderBridgeKind::Gemini => prodex_observability::ProviderKind::Gemini,
        RuntimeProviderBridgeKind::Copilot
        | RuntimeProviderBridgeKind::DeepSeek
        | RuntimeProviderBridgeKind::Kiro => prodex_observability::ProviderKind::Other,
    };
    let result = match (result, fallback_class) {
        (Err(_), _) => prodex_observability::ProviderResultClass::TransportError,
        (Ok(_), Some(ProviderErrorClass::Quota | ProviderErrorClass::RateLimit)) => {
            prodex_observability::ProviderResultClass::RateLimited
        }
        (Ok(_), Some(ProviderErrorClass::Transient)) => {
            prodex_observability::ProviderResultClass::Overloaded
        }
        (Ok(response), _) => runtime_local_rewrite_provider_result_class(response.status()),
    };
    crate::record_runtime_provider_metric(
        provider,
        result,
        duration.as_millis().try_into().unwrap_or(u64::MAX),
    );
}

pub(super) fn runtime_local_rewrite_provider_result_class(
    status: u16,
) -> prodex_observability::ProviderResultClass {
    match status {
        200..=399 => prodex_observability::ProviderResultClass::Success,
        503 => prodex_observability::ProviderResultClass::Overloaded,
        _ => prodex_observability::ProviderResultClass::ProviderError,
    }
}

pub(super) fn runtime_local_rewrite_provider_fallback_class(
    response: &RuntimeLocalRewriteUpstreamResult,
    provider: RuntimeProviderBridgeKind,
) -> Option<ProviderErrorClass> {
    match &response.response {
        RuntimeLocalRewriteUpstreamResponse::Buffered(parts) => {
            runtime_local_rewrite_buffered_fallback_class(parts, provider)
        }
        RuntimeLocalRewriteUpstreamResponse::Live(live) => {
            runtime_local_rewrite_live_fallback_class(live, provider)
        }
        _ => None,
    }
}

fn runtime_local_rewrite_buffered_fallback_class(
    parts: &RuntimeHeapTrimmedBufferedResponseParts,
    provider: RuntimeProviderBridgeKind,
) -> Option<ProviderErrorClass> {
    if parts.status < 400 {
        return None;
    }
    let class = runtime_provider_error_class(provider, parts.status, &parts.body);
    match class {
        ProviderErrorClass::Quota | ProviderErrorClass::Transient => Some(class),
        ProviderErrorClass::RateLimit
            if std::str::from_utf8(&parts.body).is_ok_and(|body| {
                let body = body.to_ascii_lowercase();
                body.contains("rate_limit_exceeded") || body.contains("rate_limit_exceeded_error")
            }) =>
        {
            Some(class)
        }
        _ => None,
    }
}

fn runtime_local_rewrite_live_fallback_class(
    live: &RuntimeLocalRewriteLiveResponse,
    provider: RuntimeProviderBridgeKind,
) -> Option<ProviderErrorClass> {
    if live.prefix.is_empty()
        || (!live.upstream_eof
            && !runtime_local_rewrite_sse_prefix_has_complete_event(&live.prefix))
    {
        return None;
    }
    let progress = if live.upstream_eof {
        runtime_proxy_crate::inspect_runtime_sse_buffer_at_eof(&live.prefix)
    } else {
        crate::runtime_proxy::inspect_runtime_sse_buffer(&live.prefix)
    };
    match progress {
        runtime_proxy_crate::RuntimeSseInspectionProgress::QuotaBlocked => {
            let class = runtime_provider_error_class(provider, live.status, &live.prefix);
            Some(
                matches!(
                    class,
                    ProviderErrorClass::Quota
                        | ProviderErrorClass::RateLimit
                        | ProviderErrorClass::Transient
                )
                .then_some(class)
                .unwrap_or(ProviderErrorClass::Quota),
            )
        }
        runtime_proxy_crate::RuntimeSseInspectionProgress::Overloaded => {
            Some(ProviderErrorClass::Transient)
        }
        _ => None,
    }
}

pub(super) fn runtime_local_rewrite_precommit_live_provider_response(
    response: &mut RuntimeLocalRewriteUpstreamResult,
    provider: RuntimeProviderBridgeKind,
    responses_route: bool,
    sse_lookahead_timeout_ms: u64,
    _stream_idle_timeout_ms: u64,
    _async_runtime: &Arc<tokio::runtime::Runtime>,
    prefetch_slots: &Arc<tokio::sync::Semaphore>,
) -> anyhow::Result<()> {
    let RuntimeLocalRewriteUpstreamResponse::Live(live) = &mut response.response else {
        return Ok(());
    };
    if !runtime_local_rewrite_should_prefetch_provider_response(live, provider, responses_route) {
        return Ok(());
    }

    let Ok(slot) = Arc::clone(prefetch_slots).try_acquire_owned() else {
        return Ok(());
    };
    let mut prefetch = live.take_sse_prefetch(Some(slot))?;
    let deadline = Instant::now() + Duration::from_millis(sse_lookahead_timeout_ms);
    let mut prefix = Vec::new();
    let mut reached_upstream_end = false;
    while prefix.len() < crate::RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let remaining_bytes = crate::RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES - prefix.len();
        match runtime_local_rewrite_process_prefetch_chunk(
            prefetch.recv_timeout(remaining),
            &mut prefetch,
            &mut prefix,
            remaining_bytes,
        )? {
            RuntimeProviderPrefetchControl::Continue => {}
            RuntimeProviderPrefetchControl::Break { upstream_end } => {
                reached_upstream_end = upstream_end;
                break;
            }
        }
    }
    live.prefix = prefix;
    live.upstream_eof = reached_upstream_end;
    live.set_sse_continuation(prefetch);
    Ok(())
}

enum RuntimeProviderPrefetchControl {
    Continue,
    Break { upstream_end: bool },
}

fn runtime_local_rewrite_process_prefetch_chunk(
    result: Result<RuntimeLocalRewritePrefetchChunk, std::sync::mpsc::RecvTimeoutError>,
    prefetch: &mut RuntimeLocalRewriteSsePrefetch,
    prefix: &mut Vec<u8>,
    remaining_bytes: usize,
) -> anyhow::Result<RuntimeProviderPrefetchControl> {
    match result {
        Ok(RuntimeLocalRewritePrefetchChunk::Data(chunk)) => {
            runtime_local_rewrite_process_prefetch_data(prefetch, prefix, remaining_bytes, chunk)
        }
        Ok(RuntimeLocalRewritePrefetchChunk::End) => {
            prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::End);
            Ok(RuntimeProviderPrefetchControl::Break { upstream_end: true })
        }
        Ok(RuntimeLocalRewritePrefetchChunk::Error(kind, message)) if prefix.is_empty() => {
            Err(anyhow::Error::new(std::io::Error::new(kind, message))
                .context("failed to read provider SSE precommit prefix"))
        }
        Ok(RuntimeLocalRewritePrefetchChunk::Error(kind, message)) => {
            prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::Error(kind, message));
            Ok(RuntimeProviderPrefetchControl::Break {
                upstream_end: false,
            })
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Ok(RuntimeProviderPrefetchControl::Break {
                upstream_end: false,
            })
        }
    }
}

fn runtime_local_rewrite_process_prefetch_data(
    prefetch: &mut RuntimeLocalRewriteSsePrefetch,
    prefix: &mut Vec<u8>,
    remaining_bytes: usize,
    chunk: Vec<u8>,
) -> anyhow::Result<RuntimeProviderPrefetchControl> {
    let inspect_len = chunk.len().min(remaining_bytes);
    let progress = runtime_local_rewrite_sse_chunk_progress(prefix, &chunk[..inspect_len]);
    let consumed = progress
        .as_ref()
        .map_or(inspect_len, |(_, consumed)| *consumed);
    prefix.extend_from_slice(&chunk[..consumed]);
    if consumed < chunk.len() {
        prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::Data(
            chunk[consumed..].to_vec(),
        ));
    }
    Ok(if progress.is_some() || consumed == remaining_bytes {
        RuntimeProviderPrefetchControl::Break {
            upstream_end: false,
        }
    } else {
        RuntimeProviderPrefetchControl::Continue
    })
}

fn runtime_local_rewrite_sse_prefix_has_complete_event(prefix: &[u8]) -> bool {
    prefix.windows(2).any(|window| window == b"\n\n")
        || prefix.windows(4).any(|window| window == b"\r\n\r\n")
}

fn runtime_local_rewrite_should_prefetch_provider_response(
    live: &RuntimeLocalRewriteLiveResponse,
    provider: RuntimeProviderBridgeKind,
    responses_route: bool,
) -> bool {
    !live.native_anthropic_messages
        && matches!(
            provider,
            RuntimeProviderBridgeKind::Anthropic
                | RuntimeProviderBridgeKind::DeepSeek
                | RuntimeProviderBridgeKind::Gemini
        )
        && responses_route
        && (200..300).contains(&live.status)
        && live
            .headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"))
        && live.prefix.is_empty()
}

fn runtime_local_rewrite_sse_event_progress(
    event: runtime_proxy_crate::RuntimeParsedSseEvent,
) -> Option<runtime_proxy_crate::RuntimeSseInspectionProgress> {
    if event.quota_blocked {
        return Some(runtime_proxy_crate::RuntimeSseInspectionProgress::QuotaBlocked);
    }
    if event.overloaded {
        return Some(runtime_proxy_crate::RuntimeSseInspectionProgress::Overloaded);
    }
    if event.previous_response_not_found {
        return Some(runtime_proxy_crate::RuntimeSseInspectionProgress::PreviousResponseNotFound);
    }
    (!event
        .event_type
        .as_deref()
        .is_some_and(runtime_proxy_crate::runtime_proxy_precommit_hold_event_kind))
    .then_some(runtime_proxy_crate::RuntimeSseInspectionProgress::Commit {
        response_ids: event.response_ids,
        turn_state: event.turn_state,
    })
}

fn runtime_local_rewrite_sse_chunk_progress(
    prefix: &[u8],
    chunk: &[u8],
) -> Option<(runtime_proxy_crate::RuntimeSseInspectionProgress, usize)> {
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    runtime_proxy_crate::runtime_sse_consume_chunk(&mut line, &mut data_lines, prefix, |_| {});
    for (index, byte) in chunk.iter().enumerate() {
        let mut progress = None;
        runtime_proxy_crate::runtime_sse_consume_chunk(
            &mut line,
            &mut data_lines,
            std::slice::from_ref(byte),
            |event| {
                progress = runtime_local_rewrite_sse_event_progress(event);
            },
        );
        if let Some(progress) = progress {
            return Some((progress, index + 1));
        }
    }
    None
}
