use anyhow::Result;
use std::time::{Duration, Instant};

use super::{
    RuntimeProfileInFlightGuard, RuntimeRotationProxyShared, RuntimeUpstreamWebSocket,
    record_runtime_profile_inflight_acquire, runtime_profile_inflight_weight,
};

#[derive(Default)]
pub(in crate::runtime_proxy) struct RuntimeWebsocketSessionState {
    upstream_socket: Option<RuntimeUpstreamWebSocket>,
    realtime_duplex: bool,
    pub(in crate::runtime_proxy) profile_name: Option<String>,
    pub(in crate::runtime_proxy) turn_state: Option<String>,
    inflight_guard: Option<RuntimeProfileInFlightGuard>,
    last_terminal_at: Option<Instant>,
    transport_generation: u64,
    last_response_transport_generation: Option<(String, u64)>,
}

impl RuntimeWebsocketSessionState {
    pub(in crate::runtime_proxy) fn with_realtime_duplex(realtime_duplex: bool) -> Self {
        Self {
            realtime_duplex,
            ..Self::default()
        }
    }

    pub(in crate::runtime_proxy) fn is_realtime_duplex(&self) -> bool {
        self.realtime_duplex
    }

    pub(in crate::runtime_proxy) fn can_reuse(
        &self,
        profile_name: &str,
        turn_state_override: Option<&str>,
    ) -> bool {
        self.upstream_socket.is_some()
            && self.profile_name.as_deref() == Some(profile_name)
            && turn_state_override.is_none_or(|value| self.turn_state.as_deref() == Some(value))
    }

    pub(in crate::runtime_proxy) fn take_socket(&mut self) -> Option<RuntimeUpstreamWebSocket> {
        self.upstream_socket.take()
    }

    pub(in crate::runtime_proxy) fn has_socket(&self) -> bool {
        self.upstream_socket.is_some()
    }

    pub(in crate::runtime_proxy) fn last_terminal_elapsed(&self) -> Option<Duration> {
        self.last_terminal_at.map(|timestamp| timestamp.elapsed())
    }

    pub(in crate::runtime_proxy) fn transport_generation(&self) -> u64 {
        self.transport_generation
    }

    pub(in crate::runtime_proxy) fn advance_transport_generation(&mut self) -> u64 {
        self.transport_generation = self.transport_generation.saturating_add(1);
        self.transport_generation
    }

    pub(in crate::runtime_proxy) fn remember_response_transport_generation(
        &mut self,
        response_ids: impl IntoIterator<Item = String>,
        transport_generation: u64,
    ) {
        if let Some(response_id) = response_ids.into_iter().last() {
            self.last_response_transport_generation = Some((response_id, transport_generation));
        }
    }

    pub(in crate::runtime_proxy) fn response_transport_generation(
        &self,
        response_id: &str,
    ) -> Option<u64> {
        self.last_response_transport_generation.as_ref().and_then(
            |(owned_response_id, generation)| {
                (owned_response_id == response_id).then_some(*generation)
            },
        )
    }

    pub(in crate::runtime_proxy) fn store(
        &mut self,
        socket: RuntimeUpstreamWebSocket,
        profile_name: &str,
        turn_state: Option<String>,
        inflight_guard: Option<RuntimeProfileInFlightGuard>,
    ) {
        self.upstream_socket = Some(socket);
        self.profile_name = Some(profile_name.to_string());
        self.turn_state = turn_state;
        self.last_terminal_at = Some(Instant::now());
        if let Some(inflight_guard) = inflight_guard {
            self.inflight_guard = Some(inflight_guard);
        }
    }

    pub(in crate::runtime_proxy) fn reset(&mut self) {
        self.upstream_socket = None;
        self.profile_name = None;
        self.turn_state = None;
        self.inflight_guard = None;
    }

    pub(in crate::runtime_proxy) fn close(&mut self) {
        if let Some(mut socket) = self.upstream_socket.take() {
            let _ = socket.close(None);
        }
        self.profile_name = None;
        self.turn_state = None;
        self.inflight_guard = None;
    }
}

#[cfg(test)]
pub(crate) fn acquire_runtime_profile_inflight_guard(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &'static str,
) -> Result<RuntimeProfileInFlightGuard> {
    try_acquire_runtime_profile_inflight_guard(shared, profile_name, context, true)?
        .ok_or_else(|| anyhow::anyhow!("unbounded profile in-flight admission was rejected"))
}

pub(crate) fn try_acquire_runtime_profile_inflight_guard(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &'static str,
    hard_affinity: bool,
) -> Result<Option<RuntimeProfileInFlightGuard>> {
    let weight = runtime_profile_inflight_weight(context);
    let hard_limit =
        (!hard_affinity).then_some(shared.runtime_config.tuning.profile_inflight_hard_limit);
    let Some(count) =
        shared
            .lane_admission
            .try_acquire_profile_inflight(profile_name, weight, hard_limit)
    else {
        return Ok(None);
    };
    record_runtime_profile_inflight_acquire(shared, profile_name, count, weight, context);
    Ok(Some(RuntimeProfileInFlightGuard {
        shared: shared.clone(),
        profile_name: profile_name.to_string(),
        context,
        weight,
    }))
}
