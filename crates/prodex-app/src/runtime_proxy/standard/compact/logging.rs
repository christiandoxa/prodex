use std::time::Instant;

pub(super) fn log_runtime_proxy_compact_candidate(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    candidate_name: &str,
    excluded_count: usize,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={request_id} transport=http compact_candidate={candidate_name} excluded_count={excluded_count}"
        ),
    );
}

use super::{RuntimeRotationProxyShared, runtime_proxy_log};

pub(super) fn log_runtime_proxy_compact_followup_owner(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    compact_followup_profile: Option<&(String, &'static str)>,
) {
    if let Some((profile_name, source)) = compact_followup_profile {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_followup_owner profile={profile_name} source={source}"
            ),
        );
    }
}

pub(super) fn runtime_proxy_compact_last_failure_kind(
    last_failure: Option<&(tiny_http::ResponseBox, bool)>,
    saw_transport_failure: bool,
) -> &'static str {
    match last_failure {
        Some((_response, true)) => "quota",
        Some((_response, false)) => "overload",
        None if saw_transport_failure => "transport",
        None => "none",
    }
}

pub(super) fn runtime_proxy_compact_final_failure_reason(
    last_failure: Option<&(tiny_http::ResponseBox, bool)>,
    saw_inflight_saturation: bool,
    saw_transport_failure: bool,
) -> Option<&'static str> {
    match last_failure {
        Some((_response, false)) => Some("overload"),
        Some((_response, true)) if saw_inflight_saturation => Some("inflight_saturation"),
        Some((_response, true)) => Some("quota"),
        None if saw_inflight_saturation => Some("inflight_saturation"),
        None if saw_transport_failure => Some("transport"),
        None => None,
    }
}

pub(super) struct RuntimeProxyCompactFinalFailureLog<'a> {
    pub(super) request_id: u64,
    pub(super) exit: &'a str,
    pub(super) reason: &'a str,
    pub(super) selection_attempts: usize,
    pub(super) selection_started_at: Instant,
    pub(super) pressure_mode: bool,
    pub(super) last_failure_kind: &'a str,
    pub(super) saw_inflight_saturation: bool,
    pub(super) saw_transport_failure: bool,
    pub(super) profile_name: Option<&'a str>,
}

pub(super) struct RuntimeProxyCompactAttemptFailureLog<'a> {
    pub(super) request_id: u64,
    pub(super) exit: &'a str,
    pub(super) reason: &'a str,
    pub(super) selection_attempts: usize,
    pub(super) selection_started_at: Instant,
    pub(super) pressure_mode: bool,
    pub(super) last_failure: Option<&'a (tiny_http::ResponseBox, bool)>,
    pub(super) saw_inflight_saturation: bool,
    pub(super) saw_transport_failure: bool,
    pub(super) profile_name: &'a str,
}

pub(super) fn log_runtime_proxy_compact_attempt_final_failure(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyCompactAttemptFailureLog<'_>,
) {
    log_runtime_proxy_compact_final_failure(
        shared,
        RuntimeProxyCompactFinalFailureLog {
            request_id: log.request_id,
            exit: log.exit,
            reason: log.reason,
            selection_attempts: log.selection_attempts,
            selection_started_at: log.selection_started_at,
            pressure_mode: log.pressure_mode,
            last_failure_kind: runtime_proxy_compact_last_failure_kind(
                log.last_failure,
                log.saw_transport_failure,
            ),
            saw_inflight_saturation: log.saw_inflight_saturation,
            saw_transport_failure: log.saw_transport_failure,
            profile_name: Some(log.profile_name),
        },
    );
}

pub(super) fn log_runtime_proxy_compact_final_failure(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyCompactFinalFailureLog<'_>,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={} transport=http compact_final_failure exit={} reason={} attempts={} elapsed_ms={} pressure_mode={} last_failure={} saw_inflight_saturation={} saw_transport_failure={} profile={}",
            log.request_id,
            log.exit,
            log.reason,
            log.selection_attempts,
            log.selection_started_at.elapsed().as_millis(),
            log.pressure_mode,
            log.last_failure_kind,
            log.saw_inflight_saturation,
            log.saw_transport_failure,
            log.profile_name.unwrap_or("-"),
        ),
    );
}
