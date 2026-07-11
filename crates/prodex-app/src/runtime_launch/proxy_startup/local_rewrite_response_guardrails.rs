use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use crate::RuntimeRotationProxyShared;
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::io::Read;
use std::path::Path;

pub(super) fn runtime_gateway_guardrail_stream_body(
    body: Box<dyn Read + Send>,
    request_id: u64,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Box<dyn Read + Send> {
    let keywords = shared
        .gateway_guardrails
        .blocked_output_keywords
        .iter()
        .map(|keyword| keyword.trim().to_ascii_lowercase())
        .filter(|keyword| !keyword.is_empty())
        .collect::<Vec<_>>();
    if keywords.is_empty() {
        return body;
    }
    Box::new(RuntimeGatewayGuardrailStreamReader {
        inner: body,
        request_id,
        runtime_shared: shared.runtime_shared.clone(),
        state_backend: shared.gateway_state_store.label().to_string(),
        keywords,
        tail: String::new(),
        blocked: false,
    })
}

struct RuntimeGatewayGuardrailStreamReader {
    inner: Box<dyn Read + Send>,
    request_id: u64,
    runtime_shared: RuntimeRotationProxyShared,
    state_backend: String,
    keywords: Vec<String>,
    tail: String,
    blocked: bool,
}

impl RuntimeGatewayGuardrailStreamReader {
    fn audit_blocked_output(&self) {
        let payload = serde_json::json!({
            "state_backend": self.state_backend,
            "details": {
                "reason": "blocked_output_keyword",
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
}

impl Read for RuntimeGatewayGuardrailStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.blocked {
            return Ok(0);
        }
        let read = self.inner.read(buf)?;
        if read == 0 {
            return Ok(0);
        }
        let text = String::from_utf8_lossy(&buf[..read]).to_ascii_lowercase();
        let combined = format!("{}{}", self.tail, text);
        if self
            .keywords
            .iter()
            .any(|keyword| combined.contains(keyword.as_str()))
        {
            self.blocked = true;
            self.audit_blocked_output();
            crate::runtime_proxy_log(
                &self.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_guardrail_stream_blocked",
                    [
                        runtime_proxy_log_field("request", self.request_id.to_string()),
                        runtime_proxy_log_field("transport", "http"),
                        runtime_proxy_log_field("reason", "blocked_output_keyword"),
                        runtime_proxy_log_field("matched_value_redacted", "true"),
                    ],
                ),
            );
            return Ok(0);
        }
        let keep = self
            .keywords
            .iter()
            .map(|keyword| keyword.len().saturating_sub(1))
            .max()
            .unwrap_or_default()
            .min(256);
        self.tail = combined
            .chars()
            .rev()
            .take(keep)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        Ok(read)
    }
}
