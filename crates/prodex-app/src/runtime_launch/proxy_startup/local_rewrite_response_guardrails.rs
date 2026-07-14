use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use crate::RuntimeRotationProxyShared;
use prodex_application::ApplicationResponseObligationPlan;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::{self, Cursor, Read};
use std::path::Path;

const RESPONSE_INSPECTION_PREFLIGHT_BYTES: usize = 4 * 1024;
const RESPONSE_INSPECTION_WINDOW_BYTES: usize = 4 * 1024;

pub(super) enum RuntimeGatewayGuardrailStreamPlan {
    Allowed(Box<dyn Read + Send>),
    Blocked(&'static str),
    AuditUnavailable,
}

pub(super) fn runtime_gateway_guardrail_stream_body(
    mut body: Box<dyn Read + Send>,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    obligations: Option<ApplicationResponseObligationPlan>,
    audit_context: Option<super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext>,
) -> io::Result<RuntimeGatewayGuardrailStreamPlan> {
    let inspector =
        RuntimeGatewayIncrementalInspector::new(&shared.gateway_guardrails.blocked_output_keywords);
    let audit = RuntimeGatewayGuardrailAudit {
        request_id,
        runtime_shared: shared.runtime_shared.clone(),
        state_backend: shared.gateway_state_store.label().to_string(),
        shared: shared.clone(),
        context: audit_context,
    };
    if obligations.is_some_and(|plan| {
        plan.enforce
            && plan.inspection_required
            && (plan.inspection_coverage == prodex_domain::InspectionCoverage::Unsupported
                || (plan.require_full_inspection
                    && plan.inspection_coverage != prodex_domain::InspectionCoverage::Full))
    }) {
        let reason = obligations
            .filter(|plan| {
                plan.require_full_inspection
                    && plan.inspection_coverage != prodex_domain::InspectionCoverage::Full
            })
            .map(|_| "response_inspection_incomplete")
            .unwrap_or("response_inspection_unsupported");
        if audit.block(reason, "precommit", "http").is_err() {
            return Ok(RuntimeGatewayGuardrailStreamPlan::AuditUnavailable);
        }
        return Ok(RuntimeGatewayGuardrailStreamPlan::Blocked(reason));
    }
    let maximum_bytes = obligations
        .filter(|plan| plan.enforce)
        .and_then(|plan| plan.maximum_output_tokens)
        .map(|tokens| {
            usize::try_from(tokens)
                .unwrap_or(usize::MAX)
                .saturating_mul(4)
        });
    if inspector.is_empty() && maximum_bytes.is_none() {
        return Ok(RuntimeGatewayGuardrailStreamPlan::Allowed(body));
    }

    let mut prefix = vec![0; RESPONSE_INSPECTION_PREFLIGHT_BYTES];
    let read = body.read(&mut prefix)?;
    prefix.truncate(read);
    let mut inspector = inspector;
    let precommit_reason = if inspector.inspect(&prefix) {
        Some("blocked_output_keyword")
    } else if maximum_bytes.is_some_and(|limit| prefix.len() > limit) {
        Some("output_token_limit_exceeded")
    } else {
        None
    };
    if let Some(reason) = precommit_reason {
        if audit.block(reason, "precommit", "http").is_err() {
            return Ok(RuntimeGatewayGuardrailStreamPlan::AuditUnavailable);
        }
        return Ok(RuntimeGatewayGuardrailStreamPlan::Blocked(reason));
    }

    Ok(RuntimeGatewayGuardrailStreamPlan::Allowed(Box::new(
        RuntimeGatewayGuardrailStreamReader {
            prefix: Cursor::new(prefix),
            inner: body,
            inspector,
            audit,
            blocked: false,
            emitted_bytes: read,
            maximum_bytes,
        },
    )))
}

pub(super) struct RuntimeGatewayIncrementalInspector {
    keywords: Vec<String>,
    tail: Vec<u8>,
    keep_bytes: usize,
}

impl RuntimeGatewayIncrementalInspector {
    pub(super) fn new(keywords: &[String]) -> Self {
        let keywords = keywords
            .iter()
            .map(|keyword| keyword.trim().to_lowercase())
            .filter(|keyword| !keyword.is_empty())
            .collect::<Vec<_>>();
        let keep_bytes = keywords
            .iter()
            .map(|keyword| keyword.len().saturating_sub(1))
            .max()
            .unwrap_or_default()
            .min(RESPONSE_INSPECTION_WINDOW_BYTES);
        Self {
            keywords,
            tail: Vec::new(),
            keep_bytes,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.keywords.is_empty()
    }

    pub(super) fn inspect(&mut self, chunk: &[u8]) -> bool {
        if chunk.is_empty() || self.keywords.is_empty() {
            return false;
        }
        let mut combined = Vec::with_capacity(self.tail.len().saturating_add(chunk.len()));
        combined.extend_from_slice(&self.tail);
        combined.extend_from_slice(chunk);
        let normalized = String::from_utf8_lossy(&combined).to_lowercase();
        let blocked = self
            .keywords
            .iter()
            .any(|keyword| normalized.contains(keyword));
        let keep_from = combined.len().saturating_sub(self.keep_bytes);
        self.tail.clear();
        self.tail.extend_from_slice(&combined[keep_from..]);
        blocked
    }
}

pub(super) fn runtime_gateway_guardrail_websocket_block(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    reason: &'static str,
) {
    RuntimeGatewayGuardrailAudit {
        request_id,
        runtime_shared: shared.runtime_shared.clone(),
        state_backend: shared.gateway_state_store.label().to_string(),
        shared: shared.clone(),
        context: None,
    }
    .block(reason, "postcommit", "websocket")
    .ok();
}

struct RuntimeGatewayGuardrailAudit {
    request_id: u64,
    runtime_shared: RuntimeRotationProxyShared,
    state_backend: String,
    shared: RuntimeLocalRewriteProxyShared,
    context: Option<super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext>,
}

impl RuntimeGatewayGuardrailAudit {
    fn block(
        &self,
        reason: &'static str,
        commit_state: &'static str,
        transport: &'static str,
    ) -> Result<(), prodex_storage::GovernanceRepositoryError> {
        if commit_state == "precommit"
            && super::local_rewrite_governance_audit::runtime_governance_audit_is_durable(
                &self.shared,
            )
            && let Some(context) = self.context.as_ref()
        {
            super::local_rewrite_governance_audit::persist_runtime_material_governance_audit(
                &self.shared,
                context,
                self.request_id,
                "response_precommit_block",
                prodex_domain::AuditOutcome::Denied,
                reason,
            )?;
        } else {
            let payload = serde_json::json!({
                "state_backend": self.state_backend,
                "details": {
                    "reason": reason,
                    "commit_state": commit_state,
                },
            });
            let default_log_dir = self
                .runtime_shared
                .log_path
                .parent()
                .unwrap_or_else(|| Path::new("."));
            let path = prodex_audit_log::audit_log_path(default_log_dir);
            let _ = prodex_audit_log::append_audit_event(
                &path,
                "gateway_data_plane",
                "response_guardrail_blocked",
                "failure",
                payload,
            );
        }
        crate::runtime_proxy_log(
            &self.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_guardrail_stream_blocked",
                [
                    runtime_proxy_log_field("request", self.request_id.to_string()),
                    runtime_proxy_log_field("transport", transport),
                    runtime_proxy_log_field("reason", reason),
                    runtime_proxy_log_field("commit_state", commit_state),
                    runtime_proxy_log_field("matched_value_redacted", "true"),
                ],
            ),
        );
        Ok(())
    }
}

struct RuntimeGatewayGuardrailStreamReader {
    prefix: Cursor<Vec<u8>>,
    inner: Box<dyn Read + Send>,
    inspector: RuntimeGatewayIncrementalInspector,
    audit: RuntimeGatewayGuardrailAudit,
    blocked: bool,
    emitted_bytes: usize,
    maximum_bytes: Option<usize>,
}

impl Read for RuntimeGatewayGuardrailStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.blocked || buf.is_empty() {
            return Ok(0);
        }
        let read = self.prefix.read(buf)?;
        if read != 0 {
            return Ok(read);
        }
        let read = self.inner.read(buf)?;
        if read == 0 {
            return Ok(0);
        }
        self.emitted_bytes = self.emitted_bytes.saturating_add(read);
        let reason = if self.inspector.inspect(&buf[..read]) {
            Some("blocked_output_keyword")
        } else if self
            .maximum_bytes
            .is_some_and(|limit| self.emitted_bytes > limit)
        {
            Some("output_token_limit_exceeded")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.blocked = true;
            self.audit.block(reason, "postcommit", "http").ok();
            return Ok(0);
        }
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeGatewayIncrementalInspector;

    #[derive(Clone, Copy)]
    enum ChunkMode {
        SseBytes,
        WebSocketText,
    }

    fn next_random(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    fn deterministic_chunks<'a>(text: &'a str, seed: &mut u64, mode: ChunkMode) -> Vec<&'a [u8]> {
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < text.len() {
            let width = 1 + usize::try_from(next_random(seed) % 4).unwrap_or_default();
            let end = match mode {
                ChunkMode::SseBytes => start.saturating_add(width).min(text.len()),
                ChunkMode::WebSocketText => {
                    let remaining = &text[start..];
                    start
                        + remaining
                            .char_indices()
                            .nth(width)
                            .map(|(offset, _)| offset)
                            .unwrap_or(remaining.len())
                }
            };
            chunks.push(&text.as_bytes()[start..end]);
            start = end;
        }
        chunks
    }

    fn inspect_commit_outcome(keyword: &str, chunks: &[&[u8]]) -> &'static str {
        let mut inspector = RuntimeGatewayIncrementalInspector::new(&[keyword.to_string()]);
        let mut committed = false;
        for chunk in chunks {
            if inspector.inspect(chunk) {
                return if committed { "postcommit" } else { "precommit" };
            }
            committed = true;
        }
        "allowed"
    }

    #[test]
    fn incremental_inspector_finds_every_chunk_boundary() {
        let keyword = "blocked-secret".to_string();
        for boundary in 1..keyword.len() {
            let mut inspector =
                RuntimeGatewayIncrementalInspector::new(std::slice::from_ref(&keyword));
            assert!(!inspector.inspect(&keyword.as_bytes()[..boundary]));
            assert!(inspector.inspect(&keyword.as_bytes()[boundary..]));
        }
    }

    #[test]
    fn incremental_inspector_handles_unicode_split_inside_codepoint() {
        let keyword = "clé-secrète".to_string();
        let text = keyword.as_bytes();
        for split in 1..text.len() {
            let mut inspector =
                RuntimeGatewayIncrementalInspector::new(std::slice::from_ref(&keyword));
            assert!(!inspector.inspect(&text[..split]), "split={split}");
            assert!(inspector.inspect(&text[split..]), "split={split}");
        }
    }

    #[test]
    fn incremental_inspector_does_not_treat_confusable_as_exact_match() {
        let mut inspector = RuntimeGatewayIncrementalInspector::new(&["api-key".to_string()]);
        assert!(!inspector.inspect("api-kеy".as_bytes()));
    }

    #[test]
    fn incremental_inspector_finds_unicode_across_three_chunks() {
        let keyword = "clé-secrète".to_string();
        let bytes = keyword.as_bytes();
        for first in 1..bytes.len() - 1 {
            for second in first + 1..bytes.len() {
                let mut inspector =
                    RuntimeGatewayIncrementalInspector::new(std::slice::from_ref(&keyword));
                assert!(!inspector.inspect(&bytes[..first]));
                assert!(!inspector.inspect(&bytes[first..second]));
                assert!(inspector.inspect(&bytes[second..]));
            }
        }
    }

    #[test]
    fn incremental_inspector_recovers_after_malformed_utf8() {
        let mut inspector =
            RuntimeGatewayIncrementalInspector::new(&["blocked-secret".to_string()]);
        assert!(!inspector.inspect(&[0xff, 0xfe]));
        assert!(inspector.inspect(b"blocked-secret"));
    }

    #[test]
    fn incremental_inspector_randomized_sse_and_websocket_chunk_corpus() {
        let corpus = [
            ("blocked-secret", "prefix:blocked-secret:suffix"),
            ("clé-secrète", "préfixe:clé-secrète:suffixe"),
            ("機密-秘密", "前置:機密-秘密:後置"),
        ];
        let mut seed = 0x5eed_cafe_f00d_u64;
        let mut split_unicode_codepoint = false;
        for mode in [ChunkMode::SseBytes, ChunkMode::WebSocketText] {
            for (keyword, text) in corpus {
                for sample in 0..128 {
                    let chunks = deterministic_chunks(text, &mut seed, mode);
                    assert!(chunks.len() > 1, "sample={sample}");
                    if matches!(mode, ChunkMode::SseBytes) {
                        split_unicode_codepoint |= chunks
                            .iter()
                            .any(|chunk| std::str::from_utf8(chunk).is_err());
                    } else {
                        assert!(
                            chunks
                                .iter()
                                .all(|chunk| std::str::from_utf8(chunk).is_ok())
                        );
                    }
                    assert!(chunks.iter().all(|chunk| {
                        !String::from_utf8_lossy(chunk)
                            .to_lowercase()
                            .contains(keyword)
                    }));
                    let mut inspector =
                        RuntimeGatewayIncrementalInspector::new(&[keyword.to_string()]);
                    assert!(
                        chunks.iter().any(|chunk| inspector.inspect(chunk)),
                        "sample={sample} keyword={keyword}"
                    );
                }
            }
        }
        assert!(split_unicode_codepoint);
    }

    #[test]
    fn incremental_inspector_commit_outcome_is_stable_across_transport_chunking() {
        let keyword = "clé-secrète";
        assert_eq!(
            inspect_commit_outcome(keyword, &[keyword.as_bytes()]),
            "precommit"
        );
        for split in 1..keyword.len() {
            assert_eq!(
                inspect_commit_outcome(
                    keyword,
                    &[&keyword.as_bytes()[..split], &keyword.as_bytes()[split..]],
                ),
                "postcommit",
                "SSE split={split}"
            );
            if keyword.is_char_boundary(split) {
                assert_eq!(
                    inspect_commit_outcome(
                        keyword,
                        &[&keyword.as_bytes()[..split], &keyword.as_bytes()[split..]],
                    ),
                    "postcommit",
                    "WebSocket split={split}"
                );
            }
        }
    }
}
