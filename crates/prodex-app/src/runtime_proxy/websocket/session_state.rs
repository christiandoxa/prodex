use anyhow::Result;
use std::time::{Duration, Instant};

use super::{
    RuntimeProfileInFlightGuard, RuntimeRotationProxyShared, RuntimeUpstreamWebSocket,
    record_runtime_profile_inflight_acquire, runtime_profile_inflight_weight,
};

#[derive(Default)]
pub(in crate::runtime_proxy) struct RuntimeWebsocketSessionState {
    upstream_socket: Option<RuntimeUpstreamWebSocket>,
    pub(in crate::runtime_proxy) profile_name: Option<String>,
    pub(in crate::runtime_proxy) turn_state: Option<String>,
    inflight_guard: Option<RuntimeProfileInFlightGuard>,
    last_terminal_at: Option<Instant>,
}

impl RuntimeWebsocketSessionState {
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

    pub(in crate::runtime_proxy) fn last_terminal_elapsed(&self) -> Option<Duration> {
        self.last_terminal_at.map(|timestamp| timestamp.elapsed())
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

pub(crate) fn acquire_runtime_profile_inflight_guard(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    context: &'static str,
) -> Result<RuntimeProfileInFlightGuard> {
    let weight = runtime_profile_inflight_weight(context);
    let count = shared
        .lane_admission
        .acquire_profile_inflight(profile_name, weight);
    record_runtime_profile_inflight_acquire(shared, profile_name, count, weight, context);
    Ok(RuntimeProfileInFlightGuard {
        shared: shared.clone(),
        profile_name: profile_name.to_string(),
        context,
        weight,
    })
}
