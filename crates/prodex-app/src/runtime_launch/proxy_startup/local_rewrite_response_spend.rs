use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_application_data_plane::runtime_gateway_application_provider_stage_is_committed;
use super::local_rewrite_transport::emit_runtime_gateway_spend_event;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_gateway_cost_for_request,
    runtime_provider_gateway_response_spend_event,
    runtime_provider_gateway_response_spend_event_from_tokens, runtime_provider_model_from_body,
};
use crate::RuntimeProxyRequest;
use prodex_domain::ReservationReconciliationReason;
use prodex_provider_core::{ProviderModelCost, estimate_text_tokens, extract_usage_tokens};
use prodex_provider_spi::ProviderRetryStage;
use std::io::Read;
use std::time::Instant;

const RUNTIME_GATEWAY_SPEND_STREAM_CAPTURE_MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeGatewayStreamTermination {
    Completed,
    Interrupted,
    Cancelled,
}

impl RuntimeGatewayStreamTermination {
    fn reconciliation_reason(self) -> ReservationReconciliationReason {
        match self {
            Self::Completed => ReservationReconciliationReason::Completed,
            Self::Interrupted => ReservationReconciliationReason::StreamInterrupted,
            Self::Cancelled => ReservationReconciliationReason::Cancelled,
        }
    }
}

pub(super) fn emit_runtime_gateway_response_spend_event_for_body(
    request_id: u64,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    status: u16,
    elapsed_ms: u128,
    response_body: &[u8],
) {
    let provider_kind = shared.provider.bridge_kind();
    let model = runtime_provider_model_from_body(&captured.body);
    let cost = runtime_gateway_response_cost(
        request_id,
        provider_kind,
        captured,
        shared,
        model.as_deref(),
    );
    emit_runtime_gateway_spend_event(
        shared,
        runtime_provider_gateway_response_spend_event(
            request_id,
            provider_kind,
            &captured.path_and_query,
            model.as_deref(),
            status,
            elapsed_ms,
            &captured.body,
            response_body,
            cost,
        ),
    );
}

fn runtime_gateway_response_cost(
    request_id: u64,
    provider_kind: RuntimeProviderBridgeKind,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    model: Option<&str>,
) -> ProviderModelCost {
    let route_load = shared
        .gateway_route_load
        .lock()
        .map(|load| load.clone())
        .unwrap_or_default();
    runtime_provider_gateway_cost_for_request(
        provider_kind,
        &shared.gateway_route_aliases,
        &route_load,
        request_id,
        &captured.body,
        model.unwrap_or("unknown"),
    )
}

pub(super) fn runtime_gateway_spend_stream_body(
    body: Box<dyn Read + Send>,
    request_id: u64,
    status: u16,
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
) -> Box<dyn Read + Send> {
    let provider_kind = shared.provider.bridge_kind();
    let model = runtime_provider_model_from_body(&captured.body);
    let cost = runtime_gateway_response_cost(
        request_id,
        provider_kind,
        captured,
        shared,
        model.as_deref(),
    );
    Box::new(RuntimeGatewaySpendStreamReader {
        inner: body,
        shared: shared.clone(),
        request_id,
        provider_kind,
        path_and_query: captured.path_and_query.clone(),
        model,
        status,
        started_at: Instant::now(),
        request_body: captured.body.clone(),
        response_bytes: 0,
        estimated_output_tokens: 0,
        captured_response: Vec::new(),
        captured_complete: true,
        cost,
        emitted: false,
        stream_committed: false,
    })
}

struct RuntimeGatewaySpendStreamReader {
    inner: Box<dyn Read + Send>,
    shared: RuntimeLocalRewriteProxyShared,
    request_id: u64,
    provider_kind: RuntimeProviderBridgeKind,
    path_and_query: String,
    model: Option<String>,
    status: u16,
    started_at: Instant,
    request_body: Vec<u8>,
    response_bytes: usize,
    estimated_output_tokens: u64,
    captured_response: Vec<u8>,
    captured_complete: bool,
    cost: ProviderModelCost,
    emitted: bool,
    stream_committed: bool,
}

impl RuntimeGatewaySpendStreamReader {
    fn observe_chunk(&mut self, chunk: &[u8]) {
        self.response_bytes = self.response_bytes.saturating_add(chunk.len());
        self.estimated_output_tokens = self
            .estimated_output_tokens
            .saturating_add(estimate_text_tokens(&String::from_utf8_lossy(chunk)));
        if self.captured_response.len() < RUNTIME_GATEWAY_SPEND_STREAM_CAPTURE_MAX_BYTES {
            let remaining =
                RUNTIME_GATEWAY_SPEND_STREAM_CAPTURE_MAX_BYTES - self.captured_response.len();
            let take = remaining.min(chunk.len());
            self.captured_response.extend_from_slice(&chunk[..take]);
            if take < chunk.len() {
                self.captured_complete = false;
            }
        } else if !chunk.is_empty() {
            self.captured_complete = false;
        }
    }

    fn commit_stream(&mut self) -> std::io::Result<()> {
        if self.stream_committed {
            return Ok(());
        }
        if !runtime_gateway_application_provider_stage_is_committed(
            ProviderRetryStage::AfterFirstByte,
        ) {
            return Err(std::io::Error::other(
                "application provider stream commit was rejected",
            ));
        }
        self.stream_committed = true;
        Ok(())
    }

    fn emit_once(&mut self, reason: ReservationReconciliationReason) {
        if self.emitted {
            return;
        }
        self.emitted = true;
        let usage = if self.captured_complete {
            extract_usage_tokens(&self.captured_response)
        } else {
            prodex_provider_core::ProviderTokenUsage::default()
        };
        let observed_output_tokens = usage
            .output_tokens
            .or_else(|| (self.estimated_output_tokens > 0).then_some(self.estimated_output_tokens));
        let mut event = runtime_provider_gateway_response_spend_event_from_tokens(
            self.request_id,
            self.provider_kind,
            &self.path_and_query,
            self.model.as_deref(),
            self.status,
            self.started_at.elapsed().as_millis(),
            &self.request_body,
            self.response_bytes,
            usage.input_tokens,
            observed_output_tokens,
            self.cost,
        );
        event.reconciliation_reason = Some(reason);
        emit_runtime_gateway_spend_event(&self.shared, event);
    }
}

impl Read for RuntimeGatewaySpendStreamReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.inner.read(buf) {
            Ok(0) => {
                self.emit_once(RuntimeGatewayStreamTermination::Completed.reconciliation_reason());
                Ok(0)
            }
            Ok(read) => {
                self.commit_stream()?;
                self.observe_chunk(&buf[..read]);
                Ok(read)
            }
            Err(err) => {
                self.emit_once(
                    RuntimeGatewayStreamTermination::Interrupted.reconciliation_reason(),
                );
                Err(err)
            }
        }
    }
}

impl Drop for RuntimeGatewaySpendStreamReader {
    fn drop(&mut self) {
        if self.emitted {
            return;
        }
        let reason = if runtime_gateway_application_provider_stage_is_committed(
            ProviderRetryStage::AfterCancellation,
        ) {
            RuntimeGatewayStreamTermination::Cancelled.reconciliation_reason()
        } else {
            RuntimeGatewayStreamTermination::Interrupted.reconciliation_reason()
        };
        self.emit_once(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_termination_preserves_completion_interruption_and_cancellation() {
        assert_eq!(
            RuntimeGatewayStreamTermination::Completed.reconciliation_reason(),
            ReservationReconciliationReason::Completed,
        );
        assert_eq!(
            RuntimeGatewayStreamTermination::Interrupted.reconciliation_reason(),
            ReservationReconciliationReason::StreamInterrupted,
        );
        assert_eq!(
            RuntimeGatewayStreamTermination::Cancelled.reconciliation_reason(),
            ReservationReconciliationReason::Cancelled,
        );
    }
}
