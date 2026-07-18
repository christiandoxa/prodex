use super::local_rewrite::RuntimeGatewayRouteLoadState;
use prodex_provider_core::estimate_request_input_tokens;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(super) type RuntimeGatewayAdaptiveQualityState = Arc<Mutex<RuntimeGatewayAdaptiveQualityStore>>;

#[derive(Clone, Copy)]
struct RuntimeGatewayAdaptiveOutcome {
    success: bool,
    latency_ms: u64,
}

#[derive(Default)]
pub(super) struct RuntimeGatewayAdaptiveQualityStore {
    windows: BTreeMap<String, VecDeque<RuntimeGatewayAdaptiveOutcome>>,
}

impl RuntimeGatewayAdaptiveQualityStore {
    pub(super) fn snapshot(
        &self,
    ) -> BTreeMap<String, runtime_proxy_crate::RuntimeGatewayAdaptiveQualityWindow> {
        self.windows
            .iter()
            .map(|(model, outcomes)| {
                let mut window =
                    runtime_proxy_crate::RuntimeGatewayAdaptiveQualityWindow::default();
                for outcome in outcomes {
                    window.record_outcome(outcome.success, outcome.latency_ms);
                }
                (model.clone(), window)
            })
            .collect()
    }

    fn record(
        &mut self,
        model: &str,
        config: runtime_proxy_crate::RuntimeGatewayAdaptiveRoutingConfig,
        outcome: RuntimeGatewayAdaptiveOutcome,
    ) {
        if !config.enabled || config.window_size == 0 {
            return;
        }
        let window = self.windows.entry(model.to_string()).or_default();
        while window.len() >= config.window_size {
            window.pop_front();
        }
        window.push_back(outcome);
    }
}

pub(super) struct RuntimeGatewayRouteLoadGuard {
    load: RuntimeGatewayRouteLoadState,
    model: String,
    started_at: Instant,
    adaptive: Option<(
        RuntimeGatewayAdaptiveQualityState,
        runtime_proxy_crate::RuntimeGatewayAdaptiveRoutingConfig,
    )>,
    success: Option<bool>,
}

impl RuntimeGatewayRouteLoadGuard {
    pub(super) fn enter(
        load: RuntimeGatewayRouteLoadState,
        model: &str,
        body: &[u8],
        adaptive_quality: RuntimeGatewayAdaptiveQualityState,
        adaptive_config: runtime_proxy_crate::RuntimeGatewayAdaptiveRoutingConfig,
    ) -> Self {
        let model = runtime_gateway_adaptive_concrete_model(model);
        let minute_epoch = runtime_gateway_route_minute_epoch();
        let estimated_tokens = estimate_request_input_tokens(body);
        if let Ok(mut load_map) = load.lock() {
            let entry = load_map.entry(model.clone()).or_default();
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
            model,
            started_at: Instant::now(),
            adaptive: adaptive_config
                .enabled
                .then_some((adaptive_quality, adaptive_config)),
            success: None,
        }
    }

    pub(super) fn mark_status(&mut self, status: u16) {
        self.success = if status < 400 {
            Some(true)
        } else if status == 408 || status == 429 || status >= 500 {
            Some(false)
        } else {
            None
        };
    }

    pub(super) fn mark_error(&mut self) {
        self.success = Some(false);
    }
}

impl Drop for RuntimeGatewayRouteLoadGuard {
    fn drop(&mut self) {
        let elapsed_ms = self
            .started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        if let Ok(mut load_map) = self.load.lock()
            && let Some(state) = load_map.get_mut(&self.model)
        {
            state.in_flight = state.in_flight.saturating_sub(1);
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
        if let (Some(success), Some((adaptive, config))) = (self.success, self.adaptive.as_ref())
            && let Ok(mut adaptive) = adaptive.lock()
        {
            adaptive.record(
                &self.model,
                *config,
                RuntimeGatewayAdaptiveOutcome {
                    success,
                    latency_ms: elapsed_ms,
                },
            );
        }
    }
}

fn runtime_gateway_adaptive_concrete_model(model: &str) -> String {
    model
        .strip_prefix("combo:")
        .and_then(|models| {
            models
                .split(',')
                .map(str::trim)
                .find(|model| !model.is_empty())
        })
        .unwrap_or(model)
        .to_string()
}

fn runtime_gateway_route_minute_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 60)
        .unwrap_or_default()
}
