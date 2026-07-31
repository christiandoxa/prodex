use super::{
    Cursor, RESPONSE_INSPECTION_READ_BYTES, RESPONSE_INSPECTION_WINDOW_BYTES, Read,
    RuntimeGatewaySpendTermination, RuntimeLocalRewriteProxyShared, RuntimeRotationProxyShared, io,
    runtime_proxy_log_field, runtime_proxy_structured_log_message,
};

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayIncrementalInspector {
    keywords: Vec<String>,
    tail: Vec<u8>,
    keep_bytes: usize,
}

impl RuntimeGatewayIncrementalInspector {
    pub(in crate::runtime_launch::proxy_startup) fn new(keywords: &[String]) -> Self {
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

    pub(in crate::runtime_launch::proxy_startup) fn is_empty(&self) -> bool {
        self.keywords.is_empty()
    }

    pub(in crate::runtime_launch::proxy_startup) fn holdback_bytes(&self) -> usize {
        self.keep_bytes
    }

    pub(in crate::runtime_launch::proxy_startup) fn inspect(&mut self, chunk: &[u8]) -> bool {
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

pub(in crate::runtime_launch::proxy_startup) fn release_safe_bytes(
    held: &mut Vec<u8>,
    chunk: &[u8],
    keep_bytes: usize,
) -> Vec<u8> {
    let mut pending = std::mem::take(held);
    pending.extend_from_slice(chunk);
    *held = pending.split_off(pending.len().saturating_sub(keep_bytes));
    pending
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_websocket_audit_context(
    authorized: Option<&prodex_application::ApplicationAuthorizedRequestContext<'_>>,
) -> Option<super::super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext> {
    authorized.and_then(
        super::super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext::from_authorized,
    )
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_guardrail_websocket_block(
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
    authorized: Option<&prodex_application::ApplicationAuthorizedRequestContext<'_>>,
    reason: &'static str,
) {
    RuntimeGatewayGuardrailAudit {
        request_id,
        runtime_shared: shared.runtime_shared.clone(),
        state_backend: shared.gateway_state_store.label().to_string(),
        shared: shared.clone(),
        context: runtime_gateway_websocket_audit_context(authorized),
    }
    .postcommit_block(reason, "websocket");
}

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayGuardrailAudit {
    pub(super) request_id: u64,
    pub(super) runtime_shared: RuntimeRotationProxyShared,
    pub(super) state_backend: String,
    pub(super) shared: RuntimeLocalRewriteProxyShared,
    pub(super) context:
        Option<super::super::local_rewrite_governance_audit::RuntimeGovernanceAuditContext>,
}

impl RuntimeGatewayGuardrailAudit {
    pub(in crate::runtime_launch::proxy_startup) fn postcommit_block(
        &self,
        reason: &str,
        transport: &'static str,
    ) {
        if self.block(reason, "postcommit", transport).is_err() {
            crate::runtime_proxy_log(
                &self.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_guardrail_postcommit_audit_failed",
                    [
                        runtime_proxy_log_field("request", self.request_id.to_string()),
                        runtime_proxy_log_field("transport", transport),
                        runtime_proxy_log_field("reason", reason),
                        runtime_proxy_log_field("error", "governance_audit_unavailable"),
                    ],
                ),
            );
        }
    }

    pub(in crate::runtime_launch::proxy_startup) fn block(
        &self,
        reason: &str,
        commit_state: &'static str,
        transport: &'static str,
    ) -> Result<(), prodex_storage::GovernanceRepositoryError> {
        let result =
            if super::super::local_rewrite_governance_audit::runtime_governance_audit_is_durable(
                &self.shared,
            ) && let Some(context) = self.context.as_ref()
            {
                if commit_state == "precommit" {
                    super::super::local_rewrite_governance_audit::persist_runtime_material_governance_audit(
                    &self.shared,
                    context,
                    self.request_id,
                    "response_precommit_block",
                    prodex_domain::AuditOutcome::Denied,
                    reason,
                )
                } else {
                    super::super::local_rewrite_governance_audit::persist_runtime_material_governance_audit_reconciling(
                    &self.shared,
                    context,
                    self.request_id,
                    "response_postcommit_block",
                    prodex_domain::AuditOutcome::Denied,
                    reason,
                )
                }
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

pub(in crate::runtime_launch::proxy_startup) struct RuntimeGatewayGuardrailStreamReader {
    pub(super) pending: Cursor<Vec<u8>>,
    pub(super) held: Vec<u8>,
    pub(super) inner: Box<dyn Read + Send>,
    pub(super) inspector: RuntimeGatewayIncrementalInspector,
    pub(super) audit: RuntimeGatewayGuardrailAudit,
    pub(super) blocked: bool,
    pub(super) eof: bool,
    pub(super) observed_bytes: usize,
    pub(super) maximum_bytes: Option<usize>,
    pub(super) termination: RuntimeGatewaySpendTermination,
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
            self.read_and_inspect_chunk()?;
            let read = self.pending.read(buf)?;
            if read != 0 || self.eof {
                return Ok(read);
            }
        }
    }
}

impl RuntimeGatewayGuardrailStreamReader {
    fn read_and_inspect_chunk(&mut self) -> io::Result<()> {
        let mut chunk = [0_u8; RESPONSE_INSPECTION_READ_BYTES];
        let read = self.inner.read(&mut chunk)?;
        if read == 0 {
            self.eof = true;
            self.pending = Cursor::new(std::mem::take(&mut self.held));
            return Ok(());
        }
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
            self.audit.postcommit_block(reason, "http");
            return Err(io::Error::other("response blocked by policy"));
        }
        self.pending = Cursor::new(release_safe_bytes(
            &mut self.held,
            &chunk[..read],
            self.inspector.holdback_bytes(),
        ));
        Ok(())
    }
}
