use super::super::super::{
    RuntimeCandidateAffinity, build_runtime_proxy_json_error_response,
    commit_runtime_proxy_profile_selection_with_notice, runtime_candidate_has_hard_affinity,
    runtime_proxy_final_retryable_http_failure_response,
    runtime_proxy_local_selection_failure_message,
};
use super::super::attempt_runtime_standard_request;
use super::logging::{
    RuntimeProxyCompactFinalFailureLog, log_runtime_proxy_compact_final_failure,
    runtime_proxy_compact_final_failure_reason, runtime_proxy_compact_last_failure_kind,
};
use crate::runtime_proxy_shared::RuntimeStandardAttempt;
use crate::runtime_state_shared::{RuntimeRotationProxyShared, RuntimeRouteKind};
use crate::shared_types::RuntimeProxyRequest;
use anyhow::Result;
use std::time::Instant;

pub(super) struct RuntimeProxyCompactSelectionExhausted<'a> {
    pub(super) request_id: u64,
    pub(super) request: &'a RuntimeProxyRequest,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) compact_owner_profile: &'a str,
    pub(super) strict_affinity_profile: Option<&'a str>,
    pub(super) session_profile: Option<&'a str>,
    pub(super) selection_attempts: usize,
    pub(super) selection_started_at: Instant,
    pub(super) pressure_mode: bool,
    pub(super) exit: &'static str,
    pub(super) fallback_exit: &'static str,
}

pub(super) fn finish_runtime_proxy_compact_selection_exhausted(
    exhausted: RuntimeProxyCompactSelectionExhausted<'_>,
    last_failure: Option<(tiny_http::ResponseBox, bool)>,
    saw_inflight_saturation: bool,
) -> Result<tiny_http::ResponseBox> {
    let final_reason =
        runtime_proxy_compact_final_failure_reason(last_failure.as_ref(), saw_inflight_saturation);
    let last_failure_kind = runtime_proxy_compact_last_failure_kind(last_failure.as_ref());
    if let Some(response) = runtime_proxy_final_retryable_http_failure_response(
        last_failure,
        saw_inflight_saturation,
        true,
    ) {
        log_runtime_proxy_compact_final_failure(
            exhausted.shared,
            RuntimeProxyCompactFinalFailureLog {
                request_id: exhausted.request_id,
                exit: exhausted.exit,
                reason: final_reason.unwrap_or("local_selection"),
                selection_attempts: exhausted.selection_attempts,
                selection_started_at: exhausted.selection_started_at,
                pressure_mode: exhausted.pressure_mode,
                last_failure_kind,
                saw_inflight_saturation,
                profile_name: None,
            },
        );
        return Ok(response);
    }
    if exhausted.strict_affinity_profile.is_some() || exhausted.session_profile.is_some() {
        log_runtime_proxy_compact_final_failure(
            exhausted.shared,
            RuntimeProxyCompactFinalFailureLog {
                request_id: exhausted.request_id,
                exit: exhausted.exit,
                reason: "local_selection",
                selection_attempts: exhausted.selection_attempts,
                selection_started_at: exhausted.selection_started_at,
                pressure_mode: exhausted.pressure_mode,
                last_failure_kind,
                saw_inflight_saturation,
                profile_name: None,
            },
        );
        return Ok(build_runtime_proxy_json_error_response(
            503,
            "service_unavailable",
            runtime_proxy_local_selection_failure_message(),
        ));
    }
    attempt_runtime_compact_owner_fallback(exhausted, last_failure_kind, saw_inflight_saturation)
}

fn attempt_runtime_compact_owner_fallback(
    exhausted: RuntimeProxyCompactSelectionExhausted<'_>,
    last_failure_kind: &'static str,
    saw_inflight_saturation: bool,
) -> Result<tiny_http::ResponseBox> {
    match attempt_runtime_standard_request(
        exhausted.request_id,
        exhausted.request,
        exhausted.shared,
        exhausted.compact_owner_profile,
        runtime_candidate_has_hard_affinity(RuntimeCandidateAffinity {
            route_kind: RuntimeRouteKind::Compact,
            candidate_name: exhausted.compact_owner_profile,
            strict_affinity_profile: exhausted.strict_affinity_profile,
            pinned_profile: None,
            turn_state_profile: None,
            session_profile: exhausted.session_profile,
            trusted_previous_response_affinity: false,
        }),
    )? {
        RuntimeStandardAttempt::Success {
            profile_name,
            response,
        } => {
            commit_runtime_proxy_profile_selection_with_notice(
                exhausted.shared,
                &profile_name,
                RuntimeRouteKind::Compact,
            )?;
            Ok(response)
        }
        RuntimeStandardAttempt::StaleContinuation { response } => Ok(response),
        RuntimeStandardAttempt::RetryableFailure {
            profile_name,
            response,
            overload,
        } => {
            log_runtime_proxy_compact_fallback_failure(
                &exhausted,
                if overload { "overload" } else { "quota" },
                last_failure_kind,
                saw_inflight_saturation,
                &profile_name,
            );
            Ok(response)
        }
        RuntimeStandardAttempt::AuthFailed {
            profile_name,
            response,
        } => {
            log_runtime_proxy_compact_fallback_failure(
                &exhausted,
                "auth",
                last_failure_kind,
                saw_inflight_saturation,
                &profile_name,
            );
            Ok(response)
        }
        RuntimeStandardAttempt::LocalSelectionBlocked { profile_name } => {
            log_runtime_proxy_compact_fallback_failure(
                &exhausted,
                "local_selection",
                last_failure_kind,
                saw_inflight_saturation,
                &profile_name,
            );
            Ok(build_runtime_proxy_json_error_response(
                503,
                "service_unavailable",
                runtime_proxy_local_selection_failure_message(),
            ))
        }
    }
}

fn log_runtime_proxy_compact_fallback_failure(
    exhausted: &RuntimeProxyCompactSelectionExhausted<'_>,
    reason: &'static str,
    last_failure_kind: &'static str,
    saw_inflight_saturation: bool,
    profile_name: &str,
) {
    log_runtime_proxy_compact_final_failure(
        exhausted.shared,
        RuntimeProxyCompactFinalFailureLog {
            request_id: exhausted.request_id,
            exit: exhausted.fallback_exit,
            reason,
            selection_attempts: exhausted.selection_attempts,
            selection_started_at: exhausted.selection_started_at,
            pressure_mode: exhausted.pressure_mode,
            last_failure_kind,
            saw_inflight_saturation,
            profile_name: Some(profile_name),
        },
    );
}
