use super::local_rewrite::{
    RuntimeLocalRewriteProxyShared, runtime_gateway_guardrail_webhook_block,
};
use super::local_rewrite_response_spend::RuntimeGatewaySpendTermination;
use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, RuntimeRotationProxyShared, runtime_proxy_log,
};
use prodex_application::ApplicationResponseObligationPlan;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::{self, Cursor, Read};

const RESPONSE_INSPECTION_PREFLIGHT_BYTES: usize = 4 * 1024;
const RESPONSE_INSPECTION_WINDOW_BYTES: usize =
    prodex_runtime_policy::MAX_GATEWAY_GUARDRAIL_KEYWORD_BYTES;
const RESPONSE_INSPECTION_READ_BYTES: usize = 8 * 1024;

pub(super) enum RuntimeGatewayGuardrailStreamPlan {
    Allowed(Box<dyn Read + Send>),
    Blocked {
        reason: &'static str,
        consumed_body: Vec<u8>,
    },
    AuditUnavailable(Vec<u8>),
}

fn runtime_gateway_fully_inspect_stream_body(
    body: &mut dyn Read,
) -> io::Result<(
    Vec<u8>,
    Option<crate::runtime_proxy::presidio::local::RuntimeLocalInspection>,
)> {
    let mut buffered = Vec::new();
    body.take((RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES as u64).saturating_add(1))
        .read_to_end(&mut buffered)?;
    let inspected = (buffered.len() <= RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES)
        .then(|| {
            crate::runtime_proxy::presidio::local::runtime_local_inspect_and_mask(buffered.clone())
                .ok()
        })
        .flatten()
        .filter(|inspected| inspected.coverage == prodex_domain::InspectionCoverage::Full);
    Ok((buffered, inspected))
}

fn runtime_gateway_response_status_is_governed(status: u16) -> bool {
    (200..300).contains(&status)
}

pub(super) fn runtime_gateway_guardrail_stream_body(
    mut body: Box<dyn Read + Send>,
    request_id: u64,
    status: u16,
    shared: &RuntimeLocalRewriteProxyShared,
    obligations: Option<ApplicationResponseObligationPlan>,
    audit_context: Option<super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext>,
    termination: RuntimeGatewaySpendTermination,
) -> io::Result<RuntimeGatewayGuardrailStreamPlan> {
    if !runtime_gateway_response_status_is_governed(status) {
        return Ok(RuntimeGatewayGuardrailStreamPlan::Allowed(body));
    }
    let inspector =
        RuntimeGatewayIncrementalInspector::new(&shared.gateway_guardrails.blocked_output_keywords);
    let audit = RuntimeGatewayGuardrailAudit {
        request_id,
        runtime_shared: shared.runtime_shared.clone(),
        state_backend: shared.gateway_state_store.label().to_string(),
        shared: shared.clone(),
        context: audit_context,
    };
    let maximum_bytes = obligations
        .filter(|plan| plan.enforce)
        .and_then(|plan| plan.maximum_output_tokens)
        .map(|tokens| {
            usize::try_from(tokens)
                .unwrap_or(usize::MAX)
                .saturating_mul(4)
        });
    if obligations.is_some_and(|plan| {
        plan.enforce
            && plan.inspection_required
            && (plan.inspection_coverage == prodex_domain::InspectionCoverage::Unsupported
                || (plan.require_full_inspection
                    && plan.inspection_coverage != prodex_domain::InspectionCoverage::Full))
    }) {
        termination.mark_policy_interrupted();
        let reason = obligations
            .filter(|plan| {
                plan.require_full_inspection
                    && plan.inspection_coverage != prodex_domain::InspectionCoverage::Full
            })
            .map(|_| "response_inspection_incomplete")
            .unwrap_or("response_inspection_unsupported");
        if audit.block(reason, "precommit", "http").is_err() {
            return Ok(RuntimeGatewayGuardrailStreamPlan::AuditUnavailable(
                Vec::new(),
            ));
        }
        return Ok(RuntimeGatewayGuardrailStreamPlan::Blocked {
            reason,
            consumed_body: Vec::new(),
        });
    }
    if obligations.is_some_and(|plan| {
        plan.enforce
            && plan.inspection_required
            && plan.require_full_inspection
            && plan.inspection_coverage == prodex_domain::InspectionCoverage::Full
    }) {
        let (buffered, inspected) = runtime_gateway_fully_inspect_stream_body(body.as_mut())?;
        let Some(inspected) = inspected else {
            termination.mark_policy_interrupted();
            let reason = "response_inspection_incomplete";
            if audit.block(reason, "precommit", "http").is_err() {
                return Ok(RuntimeGatewayGuardrailStreamPlan::AuditUnavailable(
                    buffered,
                ));
            }
            return Ok(RuntimeGatewayGuardrailStreamPlan::Blocked {
                reason,
                consumed_body: buffered,
            });
        };
        runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_response_inspection",
                [
                    runtime_proxy_log_field("request", request_id.to_string()),
                    runtime_proxy_log_field("transport", "http_stream_buffered"),
                    runtime_proxy_log_field("coverage", inspected.coverage.as_str()),
                    runtime_proxy_log_field("finding_count", inspected.findings.len().to_string()),
                    runtime_proxy_log_field("changed", inspected.changed.to_string()),
                ],
            ),
        );
        let reason = if maximum_bytes.is_some_and(|limit| buffered.len() > limit) {
            Some("output_token_limit_exceeded")
        } else {
            runtime_proxy_crate::runtime_gateway_response_guardrail_block(
                &inspected.body,
                &shared.gateway_guardrails,
            )
            .map(|block| block.kind.as_str())
        };
        if let Some(reason) = reason {
            termination.mark_policy_interrupted();
            if audit.block(reason, "precommit", "http").is_err() {
                return Ok(RuntimeGatewayGuardrailStreamPlan::AuditUnavailable(
                    buffered,
                ));
            }
            return Ok(RuntimeGatewayGuardrailStreamPlan::Blocked {
                reason,
                consumed_body: buffered,
            });
        }
        if let Some(block) =
            runtime_gateway_guardrail_webhook_block("post", request_id, &inspected.body, shared)
        {
            termination.mark_policy_interrupted();
            if audit.block(&block.reason, "precommit", "http").is_err() {
                return Ok(RuntimeGatewayGuardrailStreamPlan::AuditUnavailable(
                    buffered,
                ));
            }
            return Ok(RuntimeGatewayGuardrailStreamPlan::Blocked {
                reason: "policy_violation",
                consumed_body: buffered,
            });
        }
        return Ok(RuntimeGatewayGuardrailStreamPlan::Allowed(Box::new(
            Cursor::new(inspected.body),
        )));
    }
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
        termination.mark_policy_interrupted();
        if audit.block(reason, "precommit", "http").is_err() {
            return Ok(RuntimeGatewayGuardrailStreamPlan::AuditUnavailable(prefix));
        }
        return Ok(RuntimeGatewayGuardrailStreamPlan::Blocked {
            reason,
            consumed_body: prefix,
        });
    }

    let mut held = Vec::new();
    let prefix = release_safe_bytes(&mut held, &prefix, inspector.holdback_bytes());
    Ok(RuntimeGatewayGuardrailStreamPlan::Allowed(Box::new(
        RuntimeGatewayGuardrailStreamReader {
            pending: Cursor::new(prefix),
            held,
            inner: body,
            inspector,
            audit,
            blocked: false,
            eof: false,
            observed_bytes: read,
            maximum_bytes,
            termination,
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

    fn holdback_bytes(&self) -> usize {
        self.keep_bytes
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

fn release_safe_bytes(held: &mut Vec<u8>, chunk: &[u8], keep_bytes: usize) -> Vec<u8> {
    let mut pending = std::mem::take(held);
    pending.extend_from_slice(chunk);
    *held = pending.split_off(pending.len().saturating_sub(keep_bytes));
    pending
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
        reason: &str,
        commit_state: &'static str,
        transport: &'static str,
    ) -> Result<(), prodex_storage::GovernanceRepositoryError> {
        let result = if super::local_rewrite_governance_audit::runtime_governance_audit_is_durable(
            &self.shared,
        ) && let Some(context) = self.context.as_ref()
        {
            super::local_rewrite_governance_audit::persist_runtime_material_governance_audit(
                &self.shared,
                context,
                self.request_id,
                if commit_state == "precommit" {
                    "response_precommit_block"
                } else {
                    "response_postcommit_block"
                },
                prodex_domain::AuditOutcome::Denied,
                reason,
            )
        } else {
            let payload = serde_json::json!({
                "state_backend": self.state_backend,
                "details": {
                    "reason": reason,
                    "commit_state": commit_state,
                },
            });
            crate::audit_log::append_runtime_audit_event_best_effort(
                &self.runtime_shared,
                "gateway_data_plane",
                "response_guardrail_blocked",
                "failure",
                payload,
            );
            Ok(())
        };
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
        result
    }
}

struct RuntimeGatewayGuardrailStreamReader {
    pending: Cursor<Vec<u8>>,
    held: Vec<u8>,
    inner: Box<dyn Read + Send>,
    inspector: RuntimeGatewayIncrementalInspector,
    audit: RuntimeGatewayGuardrailAudit,
    blocked: bool,
    eof: bool,
    observed_bytes: usize,
    maximum_bytes: Option<usize>,
    termination: RuntimeGatewaySpendTermination,
}

impl Read for RuntimeGatewayGuardrailStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let read = self.pending.read(buf)?;
        if read != 0 {
            return Ok(read);
        }
        if self.blocked {
            return Err(io::Error::other("response blocked by policy"));
        }
        if self.eof {
            return Ok(0);
        }

        loop {
            let mut chunk = [0_u8; RESPONSE_INSPECTION_READ_BYTES];
            let read = self.inner.read(&mut chunk)?;
            if read == 0 {
                self.eof = true;
                self.pending = Cursor::new(std::mem::take(&mut self.held));
            } else {
                self.observed_bytes = self.observed_bytes.saturating_add(read);
                let reason = if self.inspector.inspect(&chunk[..read]) {
                    Some("blocked_output_keyword")
                } else if self
                    .maximum_bytes
                    .is_some_and(|limit| self.observed_bytes > limit)
                {
                    Some("output_token_limit_exceeded")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    self.blocked = true;
                    self.termination.mark_policy_interrupted();
                    self.audit.block(reason, "postcommit", "http").ok();
                    return Err(io::Error::other("response blocked by policy"));
                }

                self.pending = Cursor::new(release_safe_bytes(
                    &mut self.held,
                    &chunk[..read],
                    self.inspector.holdback_bytes(),
                ));
            }

            let read = self.pending.read(buf)?;
            if read != 0 || self.eof {
                return Ok(read);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeGatewayIncrementalInspector, release_safe_bytes,
        runtime_gateway_fully_inspect_stream_body, runtime_gateway_response_status_is_governed,
    };
    use std::io::Cursor;

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
    fn incremental_inspector_withholds_every_possible_keyword_prefix() {
        let keyword = b"blocked-secret";
        let mut inspector =
            RuntimeGatewayIncrementalInspector::new(&[
                String::from_utf8_lossy(keyword).into_owned()
            ]);
        let mut held = Vec::new();
        let mut released = Vec::new();
        for byte in &keyword[..keyword.len() - 1] {
            assert!(!inspector.inspect(std::slice::from_ref(byte)));
            released.extend(release_safe_bytes(
                &mut held,
                std::slice::from_ref(byte),
                inspector.holdback_bytes(),
            ));
        }
        assert!(released.is_empty());
        assert!(inspector.inspect(&keyword[keyword.len() - 1..]));
        assert_eq!(held, keyword[..keyword.len() - 1]);
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

    #[test]
    fn full_stream_inspection_buffers_and_masks_before_release() {
        let body = concat!(
            "data: {\"type\":\"response.output_text.delta\",",
            "\"delta\":\"contact user@example.com\"}\n\n",
            "data: [DONE]\n\n",
        );
        let mut body = Cursor::new(body.as_bytes());
        let (consumed, inspected) = runtime_gateway_fully_inspect_stream_body(&mut body).unwrap();
        let inspected = inspected.expect("text SSE should support full local inspection");
        let rendered = String::from_utf8(inspected.body).unwrap();

        assert_eq!(consumed, body.into_inner());
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("user@example.com"));
    }

    #[test]
    fn only_successful_streaming_responses_are_governed() {
        assert!(runtime_gateway_response_status_is_governed(200));
        assert!(!runtime_gateway_response_status_is_governed(429));
        assert!(!runtime_gateway_response_status_is_governed(500));
    }
}
