use super::{
    CONTINUATION_WEBSOCKET_INVALID_PREVIOUS_RESPONSE_PLAN, continuation_status_transition,
    selection_planning::WebsocketChainReuseReason,
    selection_planning::{
        WebsocketInvalidPreviousResponseAction, WebsocketInvalidPreviousResponsePlan,
    },
};
use crate::MojoError;

const WEBSOCKET_FAILURE_DECISION_ABI_VERSION: i64 = 2;
const WEBSOCKET_FAILURE_RECOVERY_ABI_VERSION: i64 = 1;
const WEBSOCKET_FAILURE_DECISION_FIELD_COUNT: usize = 12;

const WEBSOCKET_FAILURE_PLAN_FIELD_COUNT: usize = 5;
const WEBSOCKET_STALE_FRAME_ABI_VERSION: i64 = 1;
const WEBSOCKET_TERMINAL_FRAME_ABI_VERSION: i64 = 1;

/// Failure class whose pre-commit state transition is planned by Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketFailureKind {
    RateLimited,
    AuthFailed,
    Overloaded,
    LocalSelectionBlocked,
}

/// Side-effect instructions for one pre-commit WebSocket failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketFailureStatePlan {
    pub clear_affinity: bool,
    pub store_last_failure: bool,
    pub last_failure_retryable: bool,
    pub record_rate_limit_failure: bool,
    pub record_overload_failure: bool,
}

/// Failure classes accepted by the versioned WebSocket pre-commit decision ABI.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketFailureClass {
    RateLimited = 0,
    AuthFailed = 1,
    Overloaded = 2,
    LocalSelectionBlocked = 3,
    QuotaBlocked = 4,
    TransportFailed = 5,
    PreviousResponseNotFound = 6,
    Rejected = 7,
}

/// Host actions selected by Mojo; Rust applies the socket and state effects.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketFailureAction {
    PassThrough = 0,
    Rotate = 1,
    FullContextRetry = 2,
    RetryTransport = 3,
    ReuseWatchdog = 4,
    Continue = 5,
    Error = 6,
}

/// Deterministic WebSocket failure state and terminal-action plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketFailureDecisionPlan {
    pub action: WebsocketFailureAction,
    pub mark_backoff: bool,
    pub exclude_profile: bool,
    pub clear_affinity: bool,
    pub retryable_failure: bool,
    pub record_rate_limit_failure: bool,
    pub record_overload_failure: bool,
    pub store_last_failure: bool,
    pub last_failure_retryable: bool,
    pub release_affinity: bool,
    pub terminal: bool,
    pub reset_retry_index: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketFailureDecisionInput {
    pub failure_class: WebsocketFailureClass,
    pub stream_committed: bool,
    pub hard_affinity: bool,
    pub affinity_releasable: bool,
    pub inflight_saturated: bool,
    pub full_context_retry_available: bool,
    pub quota_fallback_available: bool,
    pub direct_current_fallback: bool,
    pub reuse_existing_session: bool,
    pub precommit_transport_retry_allowed: bool,
    pub reset_retry_index: bool,
}

/// Selects the stale-continuation WebSocket wire shape after Rust parses JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketStaleFrameAction {
    Forward,
    GenericError,
    FailedError,
    FailedResponseError,
}

unsafe extern "C" {
    fn prodex_runtime_websocket_turn_state_replay_candidate_v1(
        abi_version: i64,
        previous_response_present: i64,
        turn_state_present: i64,
        turn_state_owner_matches: i64,
        compact_followup_present: i64,
        reconstructable_full_history: i64,
    ) -> i64;
    fn prodex_runtime_websocket_context_constraint_v1(
        abi_version: i64,
        previous_response_present: i64,
        requires_previous_response_affinity: i64,
        turn_state_present: i64,
        pinned_profile_present: i64,
        turn_state_profile_present: i64,
        compact_followup_present: i64,
    ) -> i64;
    fn prodex_runtime_websocket_failure_state_plan_v1(
        failure_kind: i64,
        affinity_releasable: i64,
        output: *mut i64,
    ) -> i64;
    fn prodex_runtime_websocket_failure_decision_v2(
        abi_version: i64,
        failure_kind: i64,
        stream_committed: i64,
        hard_affinity: i64,
        affinity_releasable: i64,
        inflight_saturated: i64,
        full_context_retry_available: i64,
        quota_fallback_available: i64,
        direct_current_fallback: i64,
        reuse_existing_session: i64,
        precommit_transport_retry_allowed: i64,
        reset_retry_index: i64,
        output: *mut i64,
    ) -> i64;
    fn prodex_runtime_websocket_failure_frame_classification_v1(
        abi_version: i64,
        http_class: i64,
        http_action: i64,
        connection_limit: i64,
        previous_response_not_found: i64,
        stream_committed: i64,
    ) -> i64;
    fn prodex_runtime_websocket_invalid_id_code_patch_v1(
        abi_version: i64,
        exact_invalid: i64,
        error_object_present: i64,
        error_code_present: i64,
    ) -> i64;
    fn prodex_runtime_websocket_stale_frame_plan_v1(
        abi_version: i64,
        previous_response_message_present: i64,
        json_valid: i64,
        response_failed: i64,
        response_object_present: i64,
    ) -> i64;
    fn prodex_runtime_websocket_terminal_frame_plan_v1(
        abi_version: i64,
        event_address: u64,
        event_length: i64,
        event_present: i64,
        nested_code_address: u64,
        nested_code_length: i64,
        nested_code_present: i64,
        top_code_address: u64,
        top_code_length: i64,
        top_code_present: i64,
        status_present: i64,
        status: i64,
        status_code_present: i64,
        status_code: i64,
    ) -> i64;
}

/// Whether a quota-blocked turn-state request can safely replay from full history.
pub fn websocket_turn_state_replay_candidate(
    previous_response_present: bool,
    turn_state_present: bool,
    turn_state_owner_matches: bool,
    compact_followup_present: bool,
    reconstructable_full_history: bool,
) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_websocket_turn_state_replay_candidate_v1(
            WEBSOCKET_FAILURE_RECOVERY_ABI_VERSION,
            i64::from(previous_response_present),
            i64::from(turn_state_present),
            i64::from(turn_state_owner_matches),
            i64::from(compact_followup_present),
            i64::from(reconstructable_full_history),
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Whether a WebSocket quota fallback must preserve request context ownership.
pub fn websocket_context_constraint(
    previous_response_present: bool,
    requires_previous_response_affinity: bool,
    turn_state_present: bool,
    pinned_profile_present: bool,
    turn_state_profile_present: bool,
    compact_followup_present: bool,
) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_websocket_context_constraint_v1(
            WEBSOCKET_FAILURE_RECOVERY_ABI_VERSION,
            i64::from(previous_response_present),
            i64::from(requires_previous_response_affinity),
            i64::from(turn_state_present),
            i64::from(pinned_profile_present),
            i64::from(turn_state_profile_present),
            i64::from(compact_followup_present),
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Runs the checked versioned failure decision without a Rust fallback.
pub fn websocket_failure_decision(
    input: WebsocketFailureDecisionInput,
) -> Result<WebsocketFailureDecisionPlan, MojoError> {
    let mut output = [-1_i64; WEBSOCKET_FAILURE_DECISION_FIELD_COUNT];
    let status = unsafe {
        prodex_runtime_websocket_failure_decision_v2(
            WEBSOCKET_FAILURE_DECISION_ABI_VERSION,
            input.failure_class as i64,
            i64::from(input.stream_committed),
            i64::from(input.hard_affinity),
            i64::from(input.affinity_releasable),
            i64::from(input.inflight_saturated),
            i64::from(input.full_context_retry_available),
            i64::from(input.quota_fallback_available),
            i64::from(input.direct_current_fallback),
            i64::from(input.reuse_existing_session),
            i64::from(input.precommit_transport_retry_allowed),
            i64::from(input.reset_retry_index),
            output.as_mut_ptr(),
        )
    };
    if status == 4 {
        return Err(MojoError::AbiMismatch);
    }
    if status != 0 || output[1..].iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(if status == 1 {
            MojoError::InvalidInput
        } else {
            MojoError::InvalidOutput
        });
    }
    let action = match output[0] {
        0 => WebsocketFailureAction::PassThrough,
        1 => WebsocketFailureAction::Rotate,
        2 => WebsocketFailureAction::FullContextRetry,
        3 => WebsocketFailureAction::RetryTransport,
        4 => WebsocketFailureAction::ReuseWatchdog,
        5 => WebsocketFailureAction::Continue,
        6 => WebsocketFailureAction::Error,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(WebsocketFailureDecisionPlan {
        action,
        mark_backoff: output[1] == 1,
        exclude_profile: output[2] == 1,
        clear_affinity: output[3] == 1,
        retryable_failure: output[4] == 1,
        record_rate_limit_failure: output[5] == 1,
        record_overload_failure: output[6] == 1,
        store_last_failure: output[7] == 1,
        last_failure_retryable: output[8] == 1,
        release_affinity: output[9] == 1,
        terminal: output[10] == 1,
        reset_retry_index: output[11] == 1,
    })
}

/// Classifies a parsed WebSocket error after Rust has collected JSON facts.
pub fn websocket_failure_frame_classification(
    http_class: i64,
    http_action: i64,
    connection_limit: bool,
    previous_response_not_found: bool,
    stream_committed: bool,
) -> Result<i64, MojoError> {
    let result = unsafe {
        prodex_runtime_websocket_failure_frame_classification_v1(
            1,
            http_class,
            http_action,
            i64::from(connection_limit),
            i64::from(previous_response_not_found),
            i64::from(stream_committed),
        )
    };
    match result {
        0..=5 => Ok(result),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Facts needed to decide WebSocket error-frame retry precedence and terminality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketTerminalFramePlan {
    pub connection_limit: bool,
    pub terminal: bool,
}

fn websocket_terminal_text_parts(value: Option<&str>) -> Result<(u64, i64, i64), MojoError> {
    let Some(value) = value else {
        return Ok((0, 0, 0));
    };
    Ok((
        value.as_ptr() as usize as u64,
        i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        1,
    ))
}

/// Plans connection-limit retry precedence and wrapped error terminality.
pub fn websocket_terminal_frame_plan(
    event_type: Option<&str>,
    nested_error_code: Option<&str>,
    top_level_code: Option<&str>,
    status: Option<u16>,
    status_code: Option<u16>,
) -> Result<WebsocketTerminalFramePlan, MojoError> {
    let (event_address, event_length, event_present) = websocket_terminal_text_parts(event_type)?;
    let (nested_code_address, nested_code_length, nested_code_present) =
        websocket_terminal_text_parts(nested_error_code)?;
    let (top_code_address, top_code_length, top_code_present) =
        websocket_terminal_text_parts(top_level_code)?;
    let result = unsafe {
        prodex_runtime_websocket_terminal_frame_plan_v1(
            WEBSOCKET_TERMINAL_FRAME_ABI_VERSION,
            event_address,
            event_length,
            event_present,
            nested_code_address,
            nested_code_length,
            nested_code_present,
            top_code_address,
            top_code_length,
            top_code_present,
            i64::from(status.is_some()),
            i64::from(status.unwrap_or_default()),
            i64::from(status_code.is_some()),
            i64::from(status_code.unwrap_or_default()),
        )
    };
    match result {
        0..=3 => Ok(WebsocketTerminalFramePlan {
            connection_limit: result & 1 != 0,
            terminal: result & 2 != 0,
        }),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// The only WebSocket invalid-ID error-code normalization policy.
/// Rust remains the serde_json mutation adapter and never recomputes it.
pub fn websocket_invalid_id_code_patch(
    exact_invalid: bool,
    error_object_present: bool,
    error_code_present: bool,
) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_runtime_websocket_invalid_id_code_patch_v1(
            1,
            i64::from(exact_invalid),
            i64::from(error_object_present),
            i64::from(error_code_present),
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Plans stale-continuation framing without crossing the ABI with JSON values.
pub fn websocket_stale_frame_plan(
    previous_response_message_present: bool,
    json_valid: bool,
    response_failed: bool,
    response_object_present: bool,
) -> Result<WebsocketStaleFrameAction, MojoError> {
    let result = unsafe {
        prodex_runtime_websocket_stale_frame_plan_v1(
            WEBSOCKET_STALE_FRAME_ABI_VERSION,
            i64::from(previous_response_message_present),
            i64::from(json_valid),
            i64::from(response_failed),
            i64::from(response_object_present),
        )
    };
    match result {
        0 => Ok(WebsocketStaleFrameAction::Forward),
        1 => Ok(WebsocketStaleFrameAction::GenericError),
        2 => Ok(WebsocketStaleFrameAction::FailedError),
        3 => Ok(WebsocketStaleFrameAction::FailedResponseError),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Applies continuation-status precedence to an invalid WebSocket response id.
pub fn websocket_invalid_previous_response_recovery_plan(
    previous_response_present: bool,
    session_present: bool,
    owner_matches: bool,
    owner_generation_present: bool,
    owner_generation_matches: bool,
    recovery_available: bool,
    stream_committed: bool,
) -> Result<WebsocketInvalidPreviousResponsePlan, MojoError> {
    let output = continuation_status_transition::<5>(
        CONTINUATION_WEBSOCKET_INVALID_PREVIOUS_RESPONSE_PLAN,
        &[
            i64::from(previous_response_present),
            i64::from(session_present),
            i64::from(owner_matches),
            i64::from(owner_generation_present),
            i64::from(owner_generation_matches),
            i64::from(recovery_available),
            i64::from(stream_committed),
        ],
    )?;
    let chain_reuse_reason = match output[2] {
        0 => WebsocketChainReuseReason::UpstreamReconnect,
        1 => WebsocketChainReuseReason::BoundProfileAffinity,
        2 => WebsocketChainReuseReason::UnboundPreviousResponse,
        _ => return Err(MojoError::InvalidOutput),
    };
    let action = match output[0] {
        0 => WebsocketInvalidPreviousResponseAction::PassThrough,
        1 => WebsocketInvalidPreviousResponseAction::FullContextRetry,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(WebsocketInvalidPreviousResponsePlan {
        recovery_signal: output[3] == 1,
        crossed_transport_generation: output[1] == 1,
        chain_reuse_reason,
        action,
    })
}

/// Plans state transitions while Rust retains all WebSocket effects.
pub fn websocket_failure_state_plan(
    failure_kind: WebsocketFailureKind,
    affinity_releasable: bool,
) -> Result<WebsocketFailureStatePlan, MojoError> {
    let failure_kind = match failure_kind {
        WebsocketFailureKind::RateLimited => 0,
        WebsocketFailureKind::AuthFailed => 1,
        WebsocketFailureKind::Overloaded => 2,
        WebsocketFailureKind::LocalSelectionBlocked => 3,
    };
    let mut output = [-1_i64; WEBSOCKET_FAILURE_PLAN_FIELD_COUNT];
    let status = unsafe {
        prodex_runtime_websocket_failure_state_plan_v1(
            failure_kind,
            i64::from(affinity_releasable),
            output.as_mut_ptr(),
        )
    };
    if status != 0 || output.iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(WebsocketFailureStatePlan {
        clear_affinity: output[0] == 1,
        store_last_failure: output[1] == 1,
        last_failure_retryable: output[2] == 1,
        record_rate_limit_failure: output[3] == 1,
        record_overload_failure: output[4] == 1,
    })
}

#[cfg(test)]
#[path = "websocket_failure/tests.rs"]
mod tests;
