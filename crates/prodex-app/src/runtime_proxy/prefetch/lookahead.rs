use super::{
    RuntimePrefetchChunk, RuntimePrefetchStream, RuntimeSseInspection,
    RuntimeSseInspectionProgress, inspect_runtime_sse_buffer, runtime_proxy_log_to_path,
};
use anyhow::Result;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

async fn inspect_runtime_sse_lookahead(
    prefetch: &mut RuntimePrefetchStream,
    log_path: &Path,
    request_id: u64,
) -> Result<RuntimeSseInspection> {
    let started_at = Instant::now();
    let deadline_ms = prefetch.shared.config.stream_idle_timeout_ms;
    let polling_slice = Duration::from_millis(prefetch.shared.config.lookahead_timeout_ms.max(1));
    let mut buffered = Vec::new();
    let mut waiting_logged = false;

    loop {
        // A lookahead slice is only a wakeup interval, not a commit boundary.
        // Keep the original request replayable until actual output or an error.
        let elapsed_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        let boundary = prodex_mojo_core::sse_precommit::held_boundary(
            buffered.len(),
            prefetch.shared.config.max_buffered_bytes,
            elapsed_ms,
            deadline_ms,
            false,
        )
        .map_err(|error| anyhow::anyhow!("Mojo SSE precommit boundary failed: {error:?}"))?;
        ensure_runtime_sse_held_boundary(boundary, log_path, request_id, buffered.len())?;
        let remaining = Duration::from_millis(deadline_ms.saturating_sub(elapsed_ms));
        match prefetch
            .recv_timeout_async(polling_slice.min(remaining))
            .await
        {
            Ok(RuntimePrefetchChunk::Data(chunk)) => {
                buffered.extend_from_slice(&chunk);
                if let Some(inspection) =
                    runtime_sse_lookahead_progress(&mut buffered, log_path, request_id)
                {
                    return Ok(inspection);
                }
            }
            Ok(RuntimePrefetchChunk::End) => {
                return runtime_sse_lookahead_finish(buffered, log_path, request_id, true);
            }
            Ok(RuntimePrefetchChunk::Error(kind, message)) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_error_before_commit bytes={} kind={kind:?}",
                        buffered.len()
                    ),
                );
                return Err(anyhow::Error::new(io::Error::new(kind, message))
                    .context("failed to inspect runtime auto-rotate SSE stream"));
            }
            Err(RecvTimeoutError::Timeout) => {
                if !waiting_logged {
                    runtime_proxy_log_to_path(
                        log_path,
                        &format!(
                            "request={request_id} transport=http lookahead_waiting_for_output bytes={} committed=false",
                            buffered.len()
                        ),
                    );
                    waiting_logged = true;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(anyhow::Error::new(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "runtime SSE prefetch channel disconnected before commit and EOF",
                ))
                .context("failed to inspect runtime auto-rotate SSE stream"));
            }
        }
    }
}

fn ensure_runtime_sse_held_boundary(
    boundary: prodex_mojo_core::sse_precommit::SsePrecommitBoundary,
    log_path: &Path,
    request_id: u64,
    buffered_bytes: usize,
) -> Result<()> {
    use prodex_mojo_core::sse_precommit::SsePrecommitBoundary;
    let (kind, reason) = match boundary {
        SsePrecommitBoundary::Wait => return Ok(()),
        SsePrecommitBoundary::DeadlineExceeded => (
            io::ErrorKind::TimedOut,
            "runtime SSE stream timed out before commit-ready output",
        ),
        SsePrecommitBoundary::ByteLimitExceeded => (
            io::ErrorKind::InvalidData,
            "runtime SSE precommit metadata exceeded its bounded buffer",
        ),
        SsePrecommitBoundary::IncompleteEof => (
            io::ErrorKind::UnexpectedEof,
            "runtime SSE stream ended before commit-ready output",
        ),
    };
    runtime_proxy_log_to_path(
        log_path,
        &format!(
            "request={request_id} transport=http sse_precommit_boundary_rejected bytes={buffered_bytes} reason={boundary:?} committed=false"
        ),
    );
    Err(anyhow::Error::new(io::Error::new(kind, reason))
        .context("failed to inspect runtime auto-rotate SSE stream"))
}

fn runtime_sse_lookahead_finish(
    buffered: Vec<u8>,
    log_path: &Path,
    request_id: u64,
    upstream_eof: bool,
) -> Result<RuntimeSseInspection> {
    let progress = runtime_sse_lookahead_boundary_progress(&buffered, upstream_eof);
    match progress {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        } => {
            if !buffered.is_empty() {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_eof_commit bytes={} response_ids={}",
                        buffered.len(),
                        response_ids.len()
                    ),
                );
            }
            Ok(RuntimeSseInspection::Commit {
                prelude: buffered,
                response_ids,
                turn_state,
            })
        }
        RuntimeSseInspectionProgress::Hold { .. } => {
            let boundary = prodex_mojo_core::sse_precommit::held_boundary(
                buffered.len(),
                usize::MAX,
                0,
                1,
                upstream_eof,
            )
            .map_err(|error| anyhow::anyhow!("Mojo SSE terminal boundary failed: {error:?}"))?;
            ensure_runtime_sse_held_boundary(boundary, log_path, request_id, buffered.len())?;
            Err(anyhow::anyhow!(
                "uncommitted SSE prelude is not a completed response"
            ))
        }
        RuntimeSseInspectionProgress::QuotaBlocked => {
            Ok(RuntimeSseInspection::QuotaBlocked(buffered))
        }
        RuntimeSseInspectionProgress::RateLimited { retry_after } => {
            Ok(RuntimeSseInspection::RateLimited {
                prelude: buffered,
                retry_after,
            })
        }
        RuntimeSseInspectionProgress::Overloaded => Ok(RuntimeSseInspection::Overloaded(buffered)),
        RuntimeSseInspectionProgress::PreviousResponseNotFound => {
            Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered))
        }
    }
}

fn runtime_sse_lookahead_boundary_progress(
    buffered: &[u8],
    upstream_eof: bool,
) -> RuntimeSseInspectionProgress {
    if upstream_eof {
        runtime_proxy_crate::inspect_runtime_sse_buffer_at_eof(buffered)
    } else {
        inspect_runtime_sse_buffer(buffered)
    }
}

fn runtime_sse_lookahead_progress(
    buffered: &mut Vec<u8>,
    log_path: &Path,
    request_id: u64,
) -> Option<RuntimeSseInspection> {
    match inspect_runtime_sse_buffer(buffered) {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        } => {
            runtime_proxy_log_to_path(
                log_path,
                &format!(
                    "request={request_id} transport=http lookahead_commit bytes={} response_ids={}",
                    buffered.len(),
                    response_ids.len()
                ),
            );
            Some(RuntimeSseInspection::Commit {
                prelude: std::mem::take(buffered),
                response_ids,
                turn_state,
            })
        }
        RuntimeSseInspectionProgress::Hold { .. } => None,
        RuntimeSseInspectionProgress::QuotaBlocked => {
            runtime_proxy_log_to_path(
                log_path,
                &format!(
                    "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                    buffered.len()
                ),
            );
            Some(RuntimeSseInspection::QuotaBlocked(std::mem::take(buffered)))
        }
        RuntimeSseInspectionProgress::RateLimited { retry_after } => {
            runtime_proxy_log_to_path(
                log_path,
                &format!(
                    "request={request_id} transport=http lookahead_rate_limited bytes={} retry_after_ms={}",
                    buffered.len(),
                    retry_after.map_or(0, |delay| delay.as_millis()),
                ),
            );
            Some(RuntimeSseInspection::RateLimited {
                prelude: std::mem::take(buffered),
                retry_after,
            })
        }
        RuntimeSseInspectionProgress::Overloaded => {
            runtime_proxy_log_to_path(
                log_path,
                &format!(
                    "request={request_id} transport=http lookahead_retryable_overload bytes={}",
                    buffered.len()
                ),
            );
            Some(RuntimeSseInspection::Overloaded(std::mem::take(buffered)))
        }
        RuntimeSseInspectionProgress::PreviousResponseNotFound => {
            runtime_proxy_log_to_path(
                log_path,
                &format!(
                    "request={request_id} transport=http lookahead_retryable_signal bytes={}",
                    buffered.len()
                ),
            );
            Some(RuntimeSseInspection::PreviousResponseNotFound(
                std::mem::take(buffered),
            ))
        }
    }
}

pub(crate) async fn inspect_runtime_sse_lookahead_async(
    mut prefetch: RuntimePrefetchStream,
    log_path: PathBuf,
    request_id: u64,
) -> Result<(RuntimeSseInspection, RuntimePrefetchStream)> {
    let inspection = inspect_runtime_sse_lookahead(&mut prefetch, &log_path, request_id).await?;
    Ok((inspection, prefetch))
}

#[cfg(test)]
mod tests {
    use super::runtime_sse_lookahead_boundary_progress;
    use crate::runtime_proxy::RuntimeSseInspectionProgress;

    const PARTIAL_QUOTA_EVENT: &[u8] =
        br#"data: {"type":"response.failed","response":{"error":{"code":"insufficient_quota"}}}"#;

    #[test]
    fn timeout_or_budget_boundary_does_not_finalize_partial_sse_event() {
        assert!(matches!(
            runtime_sse_lookahead_boundary_progress(PARTIAL_QUOTA_EVENT, false),
            RuntimeSseInspectionProgress::Hold { .. }
        ));
    }

    #[test]
    fn true_upstream_eof_finalizes_partial_sse_event() {
        assert!(matches!(
            runtime_sse_lookahead_boundary_progress(PARTIAL_QUOTA_EVENT, true),
            RuntimeSseInspectionProgress::QuotaBlocked
        ));
    }
}
