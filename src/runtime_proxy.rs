use super::*;

mod admission;
mod affinity;
mod attempt_outcome;
mod buffered_response;
mod chain_log;
mod classification;
mod continuation;
mod dispatch;
mod failure_response;
mod health;
mod lifecycle;
mod lineage;
mod path;
mod payload_detection;
mod prefetch;
mod previous_response_log;
mod previous_response_orchestration;
mod profile_state;
mod quota;
mod response_forwarding;
mod responses;
mod selection;
mod selection_plan;
mod standard;
mod transport_failure;
mod upstream;
mod websocket;
mod websocket_message;

pub(crate) use self::admission::*;
pub(crate) use self::affinity::*;
pub(crate) use self::attempt_outcome::*;
pub(super) use self::buffered_response::*;
pub(crate) use self::chain_log::*;
pub(crate) use self::classification::*;
pub(super) use self::continuation::*;
pub(crate) use self::dispatch::*;
pub(crate) use self::failure_response::*;
pub(super) use self::health::*;
pub(crate) use self::lifecycle::*;
pub(super) use self::lineage::*;
pub(super) use self::path::*;
pub(super) use self::payload_detection::*;
pub(super) use self::prefetch::*;
pub(crate) use self::previous_response_log::*;
pub(crate) use self::previous_response_orchestration::*;
pub(super) use self::profile_state::*;
pub(super) use self::quota::*;
pub(crate) use self::response_forwarding::*;
#[allow(unused_imports)]
pub(super) use self::responses::attempt_runtime_responses_request;
pub(crate) use self::responses::proxy_runtime_responses_request;
pub(crate) use self::selection::*;
pub(crate) use self::selection_plan::*;
pub(crate) use self::standard::*;
pub(super) use self::transport_failure::*;
pub(crate) use self::upstream::*;
pub(crate) use self::websocket::*;
use self::websocket_message::proxy_runtime_websocket_text_message;
use self::websocket_message::runtime_profile_uncached_auth_summary_for_selection;

pub(super) fn runtime_previous_response_retry_delay(retry_index: usize) -> Option<Duration> {
    RUNTIME_PREVIOUS_RESPONSE_RETRY_DELAYS_MS
        .get(retry_index)
        .copied()
        .map(Duration::from_millis)
}

pub(super) fn runtime_proxy_precommit_budget(
    continuation: bool,
    pressure_mode: bool,
) -> (usize, Duration) {
    if continuation {
        (
            RUNTIME_PROXY_PRECOMMIT_CONTINUATION_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_CONTINUATION_BUDGET_MS),
        )
    } else if pressure_mode {
        (
            RUNTIME_PROXY_PRESSURE_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRESSURE_PRECOMMIT_BUDGET_MS),
        )
    } else {
        (
            RUNTIME_PROXY_PRECOMMIT_ATTEMPT_LIMIT,
            Duration::from_millis(RUNTIME_PROXY_PRECOMMIT_BUDGET_MS),
        )
    }
}

pub(super) fn runtime_proxy_has_continuation_priority(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
) -> bool {
    previous_response_id.is_some()
        || pinned_profile.is_some()
        || request_turn_state.is_some()
        || turn_state_profile.is_some()
        || session_profile.is_some()
}

pub(super) fn runtime_wait_affinity_owner<'a>(
    strict_affinity_profile: Option<&'a str>,
    pinned_profile: Option<&'a str>,
    turn_state_profile: Option<&'a str>,
    session_profile: Option<&'a str>,
    trusted_previous_response_affinity: bool,
) -> Option<&'a str> {
    strict_affinity_profile
        .or(turn_state_profile)
        .or_else(|| {
            trusted_previous_response_affinity
                .then_some(pinned_profile)
                .flatten()
        })
        .or(session_profile)
}

pub(super) fn runtime_noncompact_session_priority_profile<'a>(
    session_profile: Option<&'a str>,
    compact_session_profile: Option<&str>,
) -> Option<&'a str> {
    if compact_session_profile.is_some_and(|profile_name| session_profile == Some(profile_name)) {
        None
    } else {
        session_profile
    }
}

pub(super) fn runtime_proxy_allows_direct_current_profile_fallback(
    previous_response_id: Option<&str>,
    pinned_profile: Option<&str>,
    request_turn_state: Option<&str>,
    turn_state_profile: Option<&str>,
    session_profile: Option<&str>,
    saw_inflight_saturation: bool,
    saw_upstream_failure: bool,
) -> bool {
    previous_response_id.is_none()
        && pinned_profile.is_none()
        && request_turn_state.is_none()
        && turn_state_profile.is_none()
        && session_profile.is_none()
        && !saw_inflight_saturation
        && !saw_upstream_failure
}

pub(super) fn runtime_proxy_local_selection_failure_message() -> &'static str {
    "Runtime proxy could not secure a healthy upstream profile before the pre-commit retry budget was exhausted. Retry the request."
}
