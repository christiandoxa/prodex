use super::*;

pub(super) fn runtime_proxy_compact_last_failure_kind(
    last_failure: Option<&(tiny_http::ResponseBox, bool)>,
) -> &'static str {
    match last_failure {
        Some((_response, true)) => "quota",
        Some((_response, false)) => "overload",
        None => "none",
    }
}

pub(super) fn runtime_proxy_compact_final_failure_reason(
    last_failure: Option<&(tiny_http::ResponseBox, bool)>,
    saw_inflight_saturation: bool,
) -> Option<&'static str> {
    match last_failure {
        Some((_response, false)) => Some("overload"),
        Some((_response, true)) if saw_inflight_saturation => Some("inflight_saturation"),
        Some((_response, true)) => Some("quota"),
        None if saw_inflight_saturation => Some("inflight_saturation"),
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
    pub(super) profile_name: Option<&'a str>,
}

pub(super) fn log_runtime_proxy_compact_final_failure(
    shared: &RuntimeRotationProxyShared,
    log: RuntimeProxyCompactFinalFailureLog<'_>,
) {
    runtime_proxy_log(
        shared,
        format!(
            "request={} transport=http compact_final_failure exit={} reason={} attempts={} elapsed_ms={} pressure_mode={} last_failure={} saw_inflight_saturation={} profile={}",
            log.request_id,
            log.exit,
            log.reason,
            log.selection_attempts,
            log.selection_started_at.elapsed().as_millis(),
            log.pressure_mode,
            log.last_failure_kind,
            log.saw_inflight_saturation,
            log.profile_name.unwrap_or("-"),
        ),
    );
}
