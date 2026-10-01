use super::super::super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult,
};
use super::super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewritePrefetchChunk,
    RuntimeLocalRewriteSsePrefetch,
};
use super::super::super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_error_class,
};
use crate::runtime_proxy::{
    bump_runtime_profile_health_score, commit_runtime_proxy_profile_selection_with_policy,
    note_runtime_profile_transport_failure,
};
use crate::{
    RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY, RuntimeHeapTrimmedBufferedResponseParts,
    RuntimeRouteKind,
};
use prodex_mojo_core::{
    provider_constraints::{
        ProviderPrecommitHealthAction, ProviderPrecommitLiveProgress, ProviderPrecommitMetricClass,
        ProviderPrecommitSseAction, provider_precommit_buffered_fallback_class,
        provider_precommit_health_action, provider_precommit_live_fallback_class,
        provider_precommit_metric_class, provider_precommit_should_prefetch,
        provider_precommit_sse_action,
    },
    rich::ascii_casefold_contains,
};
use prodex_provider_core::ProviderErrorClass;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn runtime_provider_error_class_from_tag(tag: i64) -> ProviderErrorClass {
    match tag {
        0 => ProviderErrorClass::Auth,
        1 => ProviderErrorClass::Quota,
        2 => ProviderErrorClass::RateLimit,
        3 => ProviderErrorClass::Transient,
        4 => ProviderErrorClass::NotFound,
        5 => ProviderErrorClass::Other,
        _ => unreachable!("validated Mojo provider error class"),
    }
}

pub(super) fn runtime_local_rewrite_record_provider_health(
    shared: &RuntimeLocalRewriteProxyShared,
    profile_name: &str,
    route_kind: RuntimeRouteKind,
    result: &anyhow::Result<RuntimeLocalRewriteUpstreamResult>,
    fallback_class: Option<ProviderErrorClass>,
) {
    let (result_ok, status) = result
        .as_ref()
        .map(|response| (true, response.status()))
        .unwrap_or((false, 0));
    let action = provider_precommit_health_action(
        result_ok,
        status,
        fallback_class.map(|class| class as i64),
    )
    .expect("Mojo provider precommit health policy returned invalid output");
    match action {
        ProviderPrecommitHealthAction::TransportFailure => {
            let error = result
                .as_ref()
                .err()
                .expect("transport failure action requires upstream error");
            note_runtime_profile_transport_failure(
                &shared.runtime_shared,
                profile_name,
                route_kind,
                "governed_provider_dispatch",
                error,
            );
        }
        ProviderPrecommitHealthAction::Overload => {
            let _ = bump_runtime_profile_health_score(
                &shared.runtime_shared,
                profile_name,
                route_kind,
                RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY,
                "governed_provider_overload",
            );
        }
        ProviderPrecommitHealthAction::Commit => {
            let _ = commit_runtime_proxy_profile_selection_with_policy(
                &shared.runtime_shared,
                profile_name,
                route_kind,
                false,
            );
        }
        ProviderPrecommitHealthAction::None => {}
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
    let result_ok = result.is_ok();
    let status = result.as_ref().ok().map_or(0, |response| response.status());
    let result = match provider_precommit_metric_class(
        result_ok,
        status,
        fallback_class.map(|class| class as i64),
    )
    .expect("Mojo provider precommit metric policy returned invalid output")
    {
        ProviderPrecommitMetricClass::Success => prodex_observability::ProviderResultClass::Success,
        ProviderPrecommitMetricClass::ProviderError => {
            prodex_observability::ProviderResultClass::ProviderError
        }
        ProviderPrecommitMetricClass::RateLimited => {
            prodex_observability::ProviderResultClass::RateLimited
        }
        ProviderPrecommitMetricClass::Overloaded => {
            prodex_observability::ProviderResultClass::Overloaded
        }
        ProviderPrecommitMetricClass::TransportError => {
            prodex_observability::ProviderResultClass::TransportError
        }
    };
    crate::record_runtime_provider_metric(
        provider,
        result,
        duration.as_millis().try_into().unwrap_or(u64::MAX),
    );
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
    let class = runtime_provider_error_class(provider, parts.status, &parts.body);
    let explicit_rate_limit_marker = std::str::from_utf8(&parts.body).is_ok_and(|body| {
        ascii_casefold_contains(body, "rate_limit_exceeded")
            .expect("Mojo provider rate-limit body comparison failed")
            || ascii_casefold_contains(body, "rate_limit_exceeded_error")
                .expect("Mojo provider rate-limit body comparison failed")
    });
    provider_precommit_buffered_fallback_class(
        parts.status,
        class as i64,
        explicit_rate_limit_marker,
    )
    .expect("Mojo buffered provider fallback policy returned invalid output")
    .map(runtime_provider_error_class_from_tag)
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
    let progress = match progress {
        runtime_proxy_crate::RuntimeSseInspectionProgress::QuotaBlocked => {
            ProviderPrecommitLiveProgress::QuotaBlocked
        }
        runtime_proxy_crate::RuntimeSseInspectionProgress::RateLimited { .. } => {
            ProviderPrecommitLiveProgress::RateLimited
        }
        runtime_proxy_crate::RuntimeSseInspectionProgress::Overloaded => {
            ProviderPrecommitLiveProgress::Overloaded
        }
        _ => ProviderPrecommitLiveProgress::Other,
    };
    let class = runtime_provider_error_class(provider, live.status, &live.prefix);
    provider_precommit_live_fallback_class(progress, class as i64)
        .expect("Mojo live provider fallback policy returned invalid output")
        .map(runtime_provider_error_class_from_tag)
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
    let content_type_event_stream = live
        .headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            ascii_casefold_contains(value, "text/event-stream")
                .expect("Mojo provider SSE content-type comparison failed")
        });
    provider_precommit_should_prefetch(
        provider as i64,
        live.native_anthropic_messages,
        responses_route,
        live.status,
        content_type_event_stream,
        live.prefix.is_empty(),
    )
    .expect("Mojo provider prefetch policy returned invalid output")
}

fn runtime_local_rewrite_sse_event_progress(
    event: runtime_proxy_crate::RuntimeParsedSseEvent,
) -> Option<runtime_proxy_crate::RuntimeSseInspectionProgress> {
    let hold_event = event
        .event_type
        .as_deref()
        .is_some_and(runtime_proxy_crate::runtime_proxy_precommit_hold_event_kind);
    match provider_precommit_sse_action(
        event.quota_blocked,
        event.rate_limited,
        event.overloaded,
        event.previous_response_not_found,
        hold_event,
    )
    .expect("Mojo provider SSE progress policy returned invalid output")
    {
        ProviderPrecommitSseAction::None => None,
        ProviderPrecommitSseAction::QuotaBlocked => {
            Some(runtime_proxy_crate::RuntimeSseInspectionProgress::QuotaBlocked)
        }
        ProviderPrecommitSseAction::RateLimited => Some(
            runtime_proxy_crate::RuntimeSseInspectionProgress::RateLimited {
                retry_after: event.retry_after,
            },
        ),
        ProviderPrecommitSseAction::Overloaded => {
            Some(runtime_proxy_crate::RuntimeSseInspectionProgress::Overloaded)
        }
        ProviderPrecommitSseAction::PreviousResponseNotFound => {
            Some(runtime_proxy_crate::RuntimeSseInspectionProgress::PreviousResponseNotFound)
        }
        ProviderPrecommitSseAction::Commit => {
            Some(runtime_proxy_crate::RuntimeSseInspectionProgress::Commit {
                response_ids: event.response_ids,
                turn_state: event.turn_state,
            })
        }
    }
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
