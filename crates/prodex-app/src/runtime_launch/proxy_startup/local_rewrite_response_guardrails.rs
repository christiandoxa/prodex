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

#[path = "local_rewrite_response_guardrails/stream.rs"]
mod stream;
#[cfg(test)]
use stream::runtime_gateway_websocket_audit_context;
use stream::{
    RuntimeGatewayGuardrailAudit, RuntimeGatewayGuardrailStreamReader, release_safe_bytes,
};
pub(super) use stream::{
    RuntimeGatewayIncrementalInspector, runtime_gateway_guardrail_websocket_block,
};

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

pub(super) fn runtime_gateway_response_inspection_coverage(
    governance_mode: prodex_config::GovernanceMode,
    websocket: bool,
    streaming: bool,
    keyword_inspection: bool,
    locally_inspectable: bool,
    post_webhook: bool,
) -> prodex_domain::InspectionCoverage {
    if websocket && keyword_inspection {
        prodex_domain::InspectionCoverage::Partial
    } else if websocket {
        prodex_domain::InspectionCoverage::Unsupported
    } else if (!streaming && (keyword_inspection || post_webhook))
        || (locally_inspectable
            && (!streaming || governance_mode == prodex_config::GovernanceMode::BankEnforce))
    {
        prodex_domain::InspectionCoverage::Full
    } else if keyword_inspection {
        prodex_domain::InspectionCoverage::Partial
    } else {
        prodex_domain::InspectionCoverage::Unsupported
    }
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
    if let Some(reason) = runtime_gateway_response_inspection_failure(obligations.as_ref()) {
        return Ok(runtime_gateway_guardrail_precommit_block(
            &termination,
            &audit,
            reason,
            reason,
            Vec::new(),
        ));
    }
    if obligations.is_some_and(|plan| {
        plan.enforce
            && plan.inspection_required
            && plan.require_full_inspection
            && plan.inspection_coverage == prodex_domain::InspectionCoverage::Full
    }) {
        return runtime_gateway_guardrail_full_inspection(
            body.as_mut(),
            request_id,
            shared,
            audit,
            maximum_bytes,
            termination,
        );
    }
    runtime_gateway_guardrail_preflight(body, inspector, audit, maximum_bytes, termination)
}

fn runtime_gateway_response_inspection_failure(
    obligations: Option<&ApplicationResponseObligationPlan>,
) -> Option<&'static str> {
    let plan = obligations.filter(|plan| plan.enforce && plan.inspection_required)?;
    if plan.require_full_inspection
        && plan.inspection_coverage != prodex_domain::InspectionCoverage::Full
    {
        Some("response_inspection_incomplete")
    } else if plan.inspection_coverage == prodex_domain::InspectionCoverage::Unsupported {
        Some("response_inspection_unsupported")
    } else {
        None
    }
}

fn runtime_gateway_guardrail_precommit_block(
    termination: &RuntimeGatewaySpendTermination,
    audit: &RuntimeGatewayGuardrailAudit,
    audit_reason: &str,
    reason: &'static str,
    consumed_body: Vec<u8>,
) -> RuntimeGatewayGuardrailStreamPlan {
    termination.mark_policy_interrupted();
    if audit.block(audit_reason, "precommit", "http").is_err() {
        return RuntimeGatewayGuardrailStreamPlan::AuditUnavailable(consumed_body);
    }
    RuntimeGatewayGuardrailStreamPlan::Blocked {
        reason,
        consumed_body,
    }
}

fn runtime_gateway_guardrail_full_inspection(
    body: &mut dyn Read,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    audit: RuntimeGatewayGuardrailAudit,
    maximum_bytes: Option<usize>,
    termination: RuntimeGatewaySpendTermination,
) -> io::Result<RuntimeGatewayGuardrailStreamPlan> {
    let (buffered, inspected) = runtime_gateway_fully_inspect_stream_body(body)?;
    let Some(inspected) = inspected else {
        return Ok(runtime_gateway_guardrail_precommit_block(
            &termination,
            &audit,
            "response_inspection_incomplete",
            "response_inspection_incomplete",
            buffered,
        ));
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
        return Ok(runtime_gateway_guardrail_precommit_block(
            &termination,
            &audit,
            reason,
            reason,
            buffered,
        ));
    }
    if let Some(block) =
        runtime_gateway_guardrail_webhook_block("post", request_id, &inspected.body, shared)
    {
        return Ok(runtime_gateway_guardrail_precommit_block(
            &termination,
            &audit,
            &block.reason,
            "policy_violation",
            buffered,
        ));
    }
    Ok(RuntimeGatewayGuardrailStreamPlan::Allowed(Box::new(
        Cursor::new(inspected.body),
    )))
}

fn runtime_gateway_guardrail_preflight(
    mut body: Box<dyn Read + Send>,
    mut inspector: RuntimeGatewayIncrementalInspector,
    audit: RuntimeGatewayGuardrailAudit,
    maximum_bytes: Option<usize>,
    termination: RuntimeGatewaySpendTermination,
) -> io::Result<RuntimeGatewayGuardrailStreamPlan> {
    if inspector.is_empty() && maximum_bytes.is_none() {
        return Ok(RuntimeGatewayGuardrailStreamPlan::Allowed(body));
    }
    let mut prefix = vec![0; RESPONSE_INSPECTION_PREFLIGHT_BYTES];
    let mut read = 0;
    while read < prefix.len() {
        let next = body.read(&mut prefix[read..])?;
        if next == 0 {
            break;
        }
        read += next;
    }
    prefix.truncate(read);
    let precommit_reason = if inspector.inspect(&prefix) {
        Some("blocked_output_keyword")
    } else if maximum_bytes.is_some_and(|limit| prefix.len() > limit) {
        Some("output_token_limit_exceeded")
    } else {
        None
    };
    if let Some(reason) = precommit_reason {
        return Ok(runtime_gateway_guardrail_precommit_block(
            &termination,
            &audit,
            reason,
            reason,
            prefix,
        ));
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

#[cfg(test)]
mod tests {
    use super::{
        RuntimeGatewayIncrementalInspector, release_safe_bytes,
        runtime_gateway_fully_inspect_stream_body, runtime_gateway_response_inspection_coverage,
        runtime_gateway_response_status_is_governed, runtime_gateway_websocket_audit_context,
    };
    use prodex_application::{
        ApplicationRequestDeadline, plan_application_data_plane_authorization,
        plan_application_request_authentication_from_evidence, plan_application_request_context,
    };
    use prodex_authn::VerifiedCredentialEvidence;
    use prodex_domain::{
        CredentialScope, Principal, PrincipalId, PrincipalKind, RequestId, Role, TenantId,
    };
    use prodex_gateway_http::CanonicalRequestTarget;
    use std::io::Cursor;
    use std::time::{Duration, Instant};

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
    fn websocket_guardrail_preserves_authorized_audit_context() {
        let tenant_id = TenantId::from_uuid(uuid::Uuid::from_u128(3));
        let principal = Principal::new(
            PrincipalId::from_uuid(uuid::Uuid::from_u128(4)),
            Some(tenant_id),
            PrincipalKind::VirtualKey,
            Role::Operator,
            CredentialScope::DataPlane,
        );
        let target = CanonicalRequestTarget::parse("/v1/responses").unwrap();
        let request = plan_application_request_context(
            &target,
            RequestId::from_uuid(uuid::Uuid::from_u128(5)),
            ApplicationRequestDeadline::at(Instant::now() + Duration::from_secs(30)),
            &[],
        )
        .unwrap();
        let authenticated = plan_application_request_authentication_from_evidence(
            request,
            Some(VerifiedCredentialEvidence::Principal(principal.clone())),
            false,
        )
        .unwrap();
        let authorized = plan_application_data_plane_authorization(authenticated).unwrap();

        let context = runtime_gateway_websocket_audit_context(Some(&authorized)).unwrap();
        assert_eq!(context.tenant.tenant_id, tenant_id);
        assert_eq!(context.principal, principal);
    }

    #[test]
    fn only_successful_streaming_responses_are_governed() {
        assert!(runtime_gateway_response_status_is_governed(200));
        assert!(!runtime_gateway_response_status_is_governed(429));
        assert!(!runtime_gateway_response_status_is_governed(500));
    }

    #[test]
    fn bank_text_streams_use_full_bounded_inspection_without_upgrading_websockets() {
        assert_eq!(
            runtime_gateway_response_inspection_coverage(
                prodex_config::GovernanceMode::BankEnforce,
                false,
                true,
                false,
                true,
                false,
            ),
            prodex_domain::InspectionCoverage::Full,
        );
        assert_eq!(
            runtime_gateway_response_inspection_coverage(
                prodex_config::GovernanceMode::EnterpriseEnforce,
                false,
                true,
                false,
                true,
                false,
            ),
            prodex_domain::InspectionCoverage::Unsupported,
        );
        assert_eq!(
            runtime_gateway_response_inspection_coverage(
                prodex_config::GovernanceMode::BankEnforce,
                true,
                true,
                true,
                true,
                false,
            ),
            prodex_domain::InspectionCoverage::Partial,
        );
    }
}
