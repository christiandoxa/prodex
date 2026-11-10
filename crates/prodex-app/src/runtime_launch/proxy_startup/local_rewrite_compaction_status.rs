use crate::RuntimeProxyRequest;
use crate::runtime_core_shared::runtime_proxy_log_to_path;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
struct RequestMetadata {
    client_metadata: Option<ClientMetadata>,
}

#[derive(Deserialize)]
struct ClientMetadata {
    #[serde(rename = "x-codex-turn-metadata")]
    turn: Option<String>,
}

#[derive(Deserialize)]
struct TurnMetadata {
    request_kind: Option<String>,
}

fn is_local_compaction(request: &RuntimeProxyRequest) -> bool {
    if request.method != "POST"
        || !request
            .path_and_query
            .split('?')
            .next()
            .unwrap_or("")
            .ends_with("/responses")
    {
        return false;
    }
    serde_json::from_slice::<RequestMetadata>(&request.body)
        .ok()
        .and_then(|request| request.client_metadata?.turn)
        .and_then(|turn| serde_json::from_str::<TurnMetadata>(&turn).ok())
        .and_then(|turn| turn.request_kind)
        .as_deref()
        == Some("compaction")
}

pub(super) struct LocalCompactionStatus {
    path: PathBuf,
    request_id: u64,
    started: Instant,
    progress: tokio::task::JoinHandle<()>,
}

impl LocalCompactionStatus {
    pub(super) fn start(
        request: &RuntimeProxyRequest,
        request_id: u64,
        shared: &RuntimeRotationProxyShared,
    ) -> Option<Self> {
        if !is_local_compaction(request) {
            return None;
        }
        let path = shared.log_path.clone();
        let started = Instant::now();
        runtime_proxy_log_to_path(
            &path,
            &format!("local_compaction_started request={request_id}"),
        );
        let progress_path = path.clone();
        let progress = shared.async_runtime.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                runtime_proxy_log_to_path(
                    &progress_path,
                    &format!(
                        "local_compaction_in_progress request={request_id} elapsed_ms={}",
                        started.elapsed().as_millis()
                    ),
                );
            }
        });
        Some(Self {
            path,
            request_id,
            started,
            progress,
        })
    }
}

impl Drop for LocalCompactionStatus {
    fn drop(&mut self) {
        self.progress.abort();
        runtime_proxy_log_to_path(
            &self.path,
            &format!(
                "local_compaction_request_ended request={} elapsed_ms={}",
                self.request_id,
                self.started.elapsed().as_millis()
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_explicit_codex_compaction_metadata() {
        let mut request = RuntimeProxyRequest {
            method: "POST".into(), path_and_query: "/v1/responses".into(), headers: vec![],
            body: br#"{"client_metadata":{"x-codex-turn-metadata":"{\"request_kind\":\"compaction\"}"},"input":"secret"}"#.to_vec(),
        };
        assert!(is_local_compaction(&request));
        request.body = br#"{"input":"compaction"}"#.to_vec();
        assert!(!is_local_compaction(&request));
        request.body = br#"{"client_metadata":{"x-codex-turn-metadata":"broken"}}"#.to_vec();
        assert!(!is_local_compaction(&request));
    }
}
