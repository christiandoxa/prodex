use crate::RuntimeProxyRequest;
use crate::runtime_core_shared::runtime_proxy_log_to_path;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
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
    thread_id: Option<String>,
    session_id: Option<String>,
}

pub(super) const ORNITH_RESPONSES_MODEL: &str = "ornith-35b-heretic-iq3s";
const MAX_LOCAL_TOOL_SCHEMA_BYTES: usize = 256 * 1024;

#[derive(Default)]
pub(super) struct LocalCompactionPrefixCache {
    sequence: AtomicU64,
    last: Mutex<Option<LocalToolSchemas>>,
}

pub(super) struct LocalToolSchemas {
    sequence: u64,
    thread_id: String,
    session_id: Option<String>,
    tools: Option<Box<RawValue>>,
}

pub(super) enum LocalCompactionPrefixRequest {
    Remember(LocalToolSchemas),
    Reuse(Vec<u8>),
}

impl LocalCompactionPrefixCache {
    pub(super) fn prepare(&self, body: &[u8]) -> Option<LocalCompactionPrefixRequest> {
        let mut fields: BTreeMap<&str, &RawValue> = serde_json::from_slice(body).ok()?;
        if fields
            .get("previous_response_id")
            .is_some_and(|value| value.get() != "null")
        {
            return None;
        }
        let client: ClientMetadata =
            serde_json::from_str(fields.get("client_metadata")?.get()).ok()?;
        let metadata: TurnMetadata = serde_json::from_str(client.turn.as_deref()?).ok()?;
        let thread_id = metadata
            .thread_id
            .filter(|id| !id.is_empty() && id.len() <= 128)?;
        let session_id = metadata.session_id;
        if session_id.as_ref().is_some_and(|id| id.len() > 128) {
            return None;
        }
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let tools = fields.get("tools").copied();
        let tool_count = tools
            .filter(|value| value.get().len() <= MAX_LOCAL_TOOL_SCHEMA_BYTES)
            .and_then(|value| serde_json::from_str::<Vec<&RawValue>>(value.get()).ok())
            .map(|list| list.len());
        if metadata.request_kind.as_deref() == Some("turn") {
            return Some(LocalCompactionPrefixRequest::Remember(LocalToolSchemas {
                sequence,
                thread_id,
                session_id,
                tools: tools
                    .filter(|_| tool_count.is_some_and(|count| count > 0))
                    .map(RawValue::to_owned),
            }));
        }
        if metadata.request_kind.as_deref() != Some("compaction")
            || tool_count != Some(0)
            || !matches!(
                fields.get("tool_choice").map(|value| value.get()),
                Some(r#""auto""# | r#""none""#)
            )
        {
            return None;
        }
        let tools = self
            .last
            .lock()
            .ok()?
            .as_ref()
            .filter(|cached| {
                cached.sequence < sequence
                    && cached.thread_id == thread_id
                    && cached.session_id == session_id
            })?
            .tools
            .clone()?;
        // JSON member order is part of the rendered prefix for this backend.
        fields.insert("tools", &tools);
        fields.insert("tool_choice", serde_json::from_str(r#""none""#).ok()?);
        serde_json::to_vec(&fields)
            .ok()
            .map(LocalCompactionPrefixRequest::Reuse)
    }

    pub(super) fn remember(&self, schemas: LocalToolSchemas) {
        if let Ok(mut last) = self.last.lock()
            && last
                .as_ref()
                .is_none_or(|current| current.sequence < schemas.sequence)
        {
            *last = Some(schemas);
        }
    }
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

    #[test]
    fn preserves_original_schema_property_order() {
        let cache = LocalCompactionPrefixCache::default();
        let tools = r#"[{"type":"function","name":"fixture","parameters":{"type":"object","properties":{"z":{},"a":{}}}}]"#;
        let turn = format!(
            r#"{{"client_metadata":{{"x-codex-turn-metadata":"{{\"request_kind\":\"turn\",\"thread_id\":\"fixture\"}}"}},"tools":{tools}}}"#
        );
        let Some(LocalCompactionPrefixRequest::Remember(snapshot)) = cache.prepare(turn.as_bytes())
        else {
            panic!("normal turn should provide a tool schema snapshot");
        };
        cache.remember(snapshot);
        let compact = br#"{"client_metadata":{"x-codex-turn-metadata":"{\"request_kind\":\"compaction\",\"thread_id\":\"fixture\"}"},"tool_choice":"auto","tools":[]}"#;
        let Some(LocalCompactionPrefixRequest::Reuse(rewritten)) = cache.prepare(compact) else {
            panic!("compaction should reuse the tool schemas");
        };
        assert!(std::str::from_utf8(&rewritten).unwrap().contains(tools));
    }
}
