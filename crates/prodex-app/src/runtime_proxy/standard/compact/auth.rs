use std::collections::BTreeSet;
use std::time::Instant;

use super::super::super::{
    release_runtime_auth_failed_affinity, release_runtime_compact_lineage, runtime_proxy_log,
};
use super::{
    flow::RuntimeCompactFailureFlow,
    logging::{
        RuntimeProxyCompactAttemptFailureLog, log_runtime_proxy_compact_attempt_final_failure,
    },
};
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use anyhow::Result;

pub(super) struct RuntimeProxyCompactAuthFailure<'a> {
    pub(super) request_id: u64,
    pub(super) shared: &'a RuntimeRotationProxyShared,
    pub(super) profile_name: String,
    pub(super) response: tiny_http::ResponseBox,
    pub(super) hard_affinity: bool,
    pub(super) request_session_id: Option<&'a str>,
    pub(super) request_turn_state: Option<&'a str>,
    pub(super) compact_followup_profile: &'a mut Option<(String, &'static str)>,
    pub(super) session_profile: &'a mut Option<String>,
    pub(super) excluded_profiles: &'a mut BTreeSet<String>,
    pub(super) last_failure: &'a mut Option<(tiny_http::ResponseBox, bool)>,
    pub(super) selection_attempts: usize,
    pub(super) selection_started_at: Instant,
    pub(super) pressure_mode: bool,
    pub(super) saw_inflight_saturation: bool,
    pub(super) saw_transport_failure: bool,
}

pub(super) fn handle_runtime_proxy_compact_auth_failure(
    failure: RuntimeProxyCompactAuthFailure<'_>,
) -> Result<RuntimeCompactFailureFlow> {
    let RuntimeProxyCompactAuthFailure {
        request_id,
        shared,
        profile_name,
        response,
        hard_affinity,
        request_session_id,
        request_turn_state,
        compact_followup_profile,
        session_profile,
        excluded_profiles,
        last_failure,
        selection_attempts,
        selection_started_at,
        pressure_mode,
        saw_inflight_saturation,
        saw_transport_failure,
    } = failure;
    runtime_proxy_log(
        shared,
        format!("request={request_id} transport=http compact_auth_failed profile={profile_name}"),
    );
    if hard_affinity {
        log_runtime_proxy_compact_attempt_final_failure(
            shared,
            RuntimeProxyCompactAttemptFailureLog {
                request_id,
                exit: "hard_affinity_auth_failure",
                reason: "auth",
                selection_attempts,
                selection_started_at,
                pressure_mode,
                last_failure: last_failure.as_ref(),
                saw_inflight_saturation,
                saw_transport_failure,
                profile_name: &profile_name,
            },
        );
        return Ok(RuntimeCompactFailureFlow::Return(response));
    }
    let released_affinity = release_runtime_auth_failed_affinity(
        shared,
        &profile_name,
        None,
        None,
        request_session_id,
    )?;
    let released_compact_lineage = release_runtime_compact_lineage(
        shared,
        &profile_name,
        request_session_id,
        request_turn_state,
        "auth_failed",
    )?;
    if session_profile.as_deref() == Some(profile_name.as_str()) {
        *session_profile = None;
    }
    if compact_followup_profile
        .as_ref()
        .is_some_and(|(owner, _)| owner == &profile_name)
    {
        *compact_followup_profile = None;
    }
    if released_affinity {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http auth_failed_affinity_released profile={profile_name} route=compact"
            ),
        );
    }
    if released_compact_lineage {
        runtime_proxy_log(
            shared,
            format!(
                "request={request_id} transport=http compact_lineage_released profile={profile_name} reason=auth_failed"
            ),
        );
    }
    excluded_profiles.insert(profile_name);
    *last_failure = Some((response, true));
    Ok(RuntimeCompactFailureFlow::Retry)
}
