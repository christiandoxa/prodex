use super::{
    RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES, RuntimePrefetchChunk, RuntimePrefetchStream,
    RuntimeSseInspection, RuntimeSseInspectionProgress, inspect_runtime_sse_buffer,
    runtime_proxy_log_to_path,
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
    let deadline =
        Instant::now() + Duration::from_millis(prefetch.shared.config.lookahead_timeout_ms);
    let mut buffered = Vec::new();

    while buffered.len() < RUNTIME_PROXY_SSE_LOOKAHEAD_BYTES {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline.saturating_duration_since(now);
        match prefetch.recv_timeout_async(remaining).await {
            Ok(RuntimePrefetchChunk::Data(chunk)) => {
                buffered.extend_from_slice(&chunk);
                if let Some(inspection) =
                    runtime_sse_lookahead_progress(&mut buffered, log_path, request_id)
                {
                    return Ok(inspection);
                }
            }
            Ok(RuntimePrefetchChunk::End) => break,
            Ok(RuntimePrefetchChunk::Error(kind, message)) => {
                if buffered.is_empty() {
                    runtime_proxy_log_to_path(
                        log_path,
                        &format!(
                            "request={request_id} transport=http lookahead_error_before_bytes kind={kind:?} error={message}"
                        ),
                    );
                    return Err(anyhow::Error::new(io::Error::new(kind, message))
                        .context("failed to inspect runtime auto-rotate SSE stream"));
                }
                prefetch.push_backlog(RuntimePrefetchChunk::Error(kind, message));
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_timeout bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_channel_disconnected bytes={}",
                        buffered.len()
                    ),
                );
                break;
            }
        }
    }

    runtime_sse_lookahead_finish(buffered, log_path, request_id)
}

fn runtime_sse_lookahead_finish(
    buffered: Vec<u8>,
    log_path: &Path,
    request_id: u64,
) -> Result<RuntimeSseInspection> {
    match inspect_runtime_sse_buffer(&buffered) {
        RuntimeSseInspectionProgress::Commit {
            response_ids,
            turn_state,
        }
        | RuntimeSseInspectionProgress::Hold {
            response_ids,
            turn_state,
        } => {
            if !buffered.is_empty() {
                runtime_proxy_log_to_path(
                    log_path,
                    &format!(
                        "request={request_id} transport=http lookahead_budget_exhausted bytes={} response_ids={}",
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
        RuntimeSseInspectionProgress::QuotaBlocked => {
            Ok(RuntimeSseInspection::QuotaBlocked(buffered))
        }
        RuntimeSseInspectionProgress::Overloaded => Ok(RuntimeSseInspection::Overloaded(buffered)),
        RuntimeSseInspectionProgress::PreviousResponseNotFound => {
            Ok(RuntimeSseInspection::PreviousResponseNotFound(buffered))
        }
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
