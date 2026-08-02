use super::super::super::local_rewrite_upstream::{
    RuntimeLocalRewriteLiveResponse, RuntimeLocalRewritePrefetchChunk,
};
use super::super::{
    RuntimeLocalRewriteProxyShared, RuntimeLocalRewriteUpstreamResponse,
    RuntimeLocalRewriteUpstreamResult, RuntimeProviderBridgeKind, runtime_provider_error_class,
};
use crate::runtime_proxy::{
    bump_runtime_profile_health_score, commit_runtime_proxy_profile_selection_with_policy,
    note_runtime_profile_transport_failure,
};
use crate::{RUNTIME_PROFILE_OVERLOAD_HEALTH_PENALTY, RuntimeRouteKind};
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

fn runtime_local_rewrite_provider_result_class(
    status: u16,
) -> prodex_observability::ProviderResultClass {
    match status {
        200..=399 => prodex_observability::ProviderResultClass::Success,
        429 => prodex_observability::ProviderResultClass::RateLimited,
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
            if parts.status < 400 {
                return None;
            }
            let class = runtime_provider_error_class(provider, parts.status, &parts.body);
            match class {
                ProviderErrorClass::Quota | ProviderErrorClass::Transient => Some(class),
                ProviderErrorClass::RateLimit
                    if std::str::from_utf8(&parts.body).is_ok_and(|body| {
                        let body = body.to_ascii_lowercase();
                        body.contains("rate_limit_exceeded")
                            || body.contains("rate_limit_exceeded_error")
                    }) =>
                {
                    Some(class)
                }
                _ => None,
            }
        }
        RuntimeLocalRewriteUpstreamResponse::Live(live) if !live.prefix.is_empty() => {
            match crate::runtime_proxy::inspect_runtime_sse_buffer(&live.prefix) {
                runtime_proxy_crate::RuntimeSseInspectionProgress::QuotaBlocked => {
                    let class = runtime_provider_error_class(provider, live.status, &live.prefix);
                    if matches!(
                        class,
                        ProviderErrorClass::Quota
                            | ProviderErrorClass::RateLimit
                            | ProviderErrorClass::Transient
                    ) {
                        Some(class)
                    } else {
                        Some(ProviderErrorClass::Quota)
                    }
                }
                runtime_proxy_crate::RuntimeSseInspectionProgress::Overloaded => {
                    Some(ProviderErrorClass::Transient)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn runtime_local_rewrite_precommit_live_provider_response(
    response: &mut RuntimeLocalRewriteUpstreamResult,
    provider: RuntimeProviderBridgeKind,
    responses_route: bool,
    sse_lookahead_timeout_ms: u64,
    stream_idle_timeout_ms: u64,
    async_runtime: &Arc<tokio::runtime::Runtime>,
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
    let mut prefetch = live.take_sse_prefetch(async_runtime, stream_idle_timeout_ms, slot)?;
    let deadline = Instant::now() + Duration::from_millis(sse_lookahead_timeout_ms);
    let mut prefix = Vec::new();
    while prefix.len() < crate::RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match prefetch.recv_timeout(remaining) {
            Ok(RuntimeLocalRewritePrefetchChunk::Data(mut chunk)) => {
                let remaining_bytes = crate::RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES - prefix.len();
                if chunk.len() > remaining_bytes {
                    prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::Data(
                        chunk.split_off(remaining_bytes),
                    ));
                }
                prefix.extend_from_slice(&chunk);
                if !matches!(
                    crate::runtime_proxy::inspect_runtime_sse_buffer(&prefix),
                    runtime_proxy_crate::RuntimeSseInspectionProgress::Hold { .. }
                ) {
                    break;
                }
            }
            Ok(RuntimeLocalRewritePrefetchChunk::End) => break,
            Ok(RuntimeLocalRewritePrefetchChunk::Error(kind, message)) => {
                if prefix.is_empty() {
                    return Err(anyhow::Error::new(std::io::Error::new(kind, message))
                        .context("failed to read provider SSE precommit prefix"));
                }
                prefetch.push_backlog(RuntimeLocalRewritePrefetchChunk::Error(kind, message));
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    live.prefix = prefix;
    live.set_sse_continuation(prefetch);
    Ok(())
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
