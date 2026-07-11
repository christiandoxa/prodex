use super::local_rewrite::RuntimeGatewayRouteLoadState;
use prodex_provider_core::estimate_request_input_tokens;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(super) struct RuntimeGatewayRouteLoadGuard {
    load: RuntimeGatewayRouteLoadState,
    model: String,
    started_at: Instant,
}

impl RuntimeGatewayRouteLoadGuard {
    pub(super) fn enter(load: RuntimeGatewayRouteLoadState, model: &str, body: &[u8]) -> Self {
        let minute_epoch = runtime_gateway_route_minute_epoch();
        let estimated_tokens = estimate_request_input_tokens(body);
        if let Ok(mut load_map) = load.lock() {
            let entry = load_map.entry(model.to_string()).or_default();
            if entry.minute_epoch != minute_epoch {
                entry.minute_epoch = minute_epoch;
                entry.requests_this_minute = 0;
                entry.tokens_this_minute = 0;
            }
            entry.in_flight = entry.in_flight.saturating_add(1);
            entry.requests_this_minute = entry.requests_this_minute.saturating_add(1);
            entry.tokens_this_minute = entry.tokens_this_minute.saturating_add(estimated_tokens);
        }
        Self {
            load,
            model: model.to_string(),
            started_at: Instant::now(),
        }
    }
}

impl Drop for RuntimeGatewayRouteLoadGuard {
    fn drop(&mut self) {
        if let Ok(mut load_map) = self.load.lock()
            && let Some(state) = load_map.get_mut(&self.model)
        {
            state.in_flight = state.in_flight.saturating_sub(1);
            let elapsed_ms = self
                .started_at
                .elapsed()
                .as_millis()
                .min(u128::from(u64::MAX)) as u64;
            state.latency_ms_ewma = Some(match state.latency_ms_ewma {
                Some(previous) => previous.saturating_mul(7).saturating_add(elapsed_ms) / 8,
                None => elapsed_ms,
            });
            if state.in_flight == 0
                && state.requests_this_minute == 0
                && state.tokens_this_minute == 0
            {
                load_map.remove(&self.model);
            }
        }
    }
}

fn runtime_gateway_route_minute_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 60)
        .unwrap_or_default()
}
