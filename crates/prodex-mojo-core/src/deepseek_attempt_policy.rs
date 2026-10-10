//! Mojo-owned DeepSeek pre-commit attempt precedence.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_deepseek_first_event_retry_allowed_v1(
        abi_version: i64,
        attempted_retries: i64,
        first_event_committed: i64,
    ) -> i64;
    fn prodex_deepseek_attempt_action_v1(
        abi_version: i64,
        attempt_kind: i64,
        attempted_first_event_retries: i64,
        first_event_committed: i64,
        model_index: i64,
        model_count: i64,
        credential_index: i64,
        credential_count: i64,
        model_retry_allowed: i64,
        credential_retry_allowed: i64,
    ) -> i64;
}

/// The phase of the bounded DeepSeek attempt decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum DeepSeekAttemptKind {
    NativeFirstEvent = 0,
    Error = 1,
}

/// Bounded facts supplied to the DeepSeek pre-commit attempt planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepSeekAttemptInput {
    pub kind: DeepSeekAttemptKind,
    pub attempted_first_event_retries: u8,
    pub first_event_committed: bool,
    pub model_index: usize,
    pub model_count: usize,
    pub credential_index: usize,
    pub credential_count: usize,
    pub model_retry_allowed: bool,
    pub credential_retry_allowed: bool,
}

/// The effect selected before any model output is committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSeekAttemptAction {
    Return,
    NextModel,
    NextCredential,
}

/// Decide whether the one allowed native first-event retry remains available.
pub fn first_event_retry_allowed(
    attempted_retries: u8,
    first_event_committed: bool,
) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_deepseek_first_event_retry_allowed_v1(
            ABI_VERSION,
            i64::from(attempted_retries),
            i64::from(first_event_committed),
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select model-before-credential recovery precedence from bounded attempt facts.
pub fn attempt_action(input: DeepSeekAttemptInput) -> Result<DeepSeekAttemptAction, MojoError> {
    let model_index = i64::try_from(input.model_index).map_err(|_| MojoError::InvalidInput)?;
    let model_count = i64::try_from(input.model_count).map_err(|_| MojoError::InvalidInput)?;
    let credential_index =
        i64::try_from(input.credential_index).map_err(|_| MojoError::InvalidInput)?;
    let credential_count =
        i64::try_from(input.credential_count).map_err(|_| MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_deepseek_attempt_action_v1(
            ABI_VERSION,
            input.kind as i64,
            i64::from(input.attempted_first_event_retries),
            i64::from(input.first_event_committed),
            model_index,
            model_count,
            credential_index,
            credential_count,
            i64::from(input.model_retry_allowed),
            i64::from(input.credential_retry_allowed),
        )
    };
    match result {
        0 => Ok(DeepSeekAttemptAction::Return),
        1 => Ok(DeepSeekAttemptAction::NextModel),
        2 => Ok(DeepSeekAttemptAction::NextCredential),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_event_budget_is_single_use_and_precommit_only() {
        assert!(first_event_retry_allowed(0, false).unwrap());
        assert!(!first_event_retry_allowed(1, false).unwrap());
        assert!(!first_event_retry_allowed(0, true).unwrap());
    }

    #[test]
    fn native_first_event_prefers_model_then_credential() {
        assert_eq!(
            attempt_action(DeepSeekAttemptInput {
                kind: DeepSeekAttemptKind::NativeFirstEvent,
                attempted_first_event_retries: 0,
                first_event_committed: false,
                model_index: 0,
                model_count: 2,
                credential_index: 0,
                credential_count: 2,
                model_retry_allowed: true,
                credential_retry_allowed: true,
            },)
            .unwrap(),
            DeepSeekAttemptAction::NextModel
        );
        assert_eq!(
            attempt_action(DeepSeekAttemptInput {
                kind: DeepSeekAttemptKind::NativeFirstEvent,
                attempted_first_event_retries: 0,
                first_event_committed: false,
                model_index: 1,
                model_count: 2,
                credential_index: 0,
                credential_count: 2,
                model_retry_allowed: true,
                credential_retry_allowed: true,
            },)
            .unwrap(),
            DeepSeekAttemptAction::NextCredential
        );
        assert_eq!(
            attempt_action(DeepSeekAttemptInput {
                kind: DeepSeekAttemptKind::NativeFirstEvent,
                attempted_first_event_retries: 1,
                first_event_committed: false,
                model_index: 0,
                model_count: 2,
                credential_index: 0,
                credential_count: 2,
                model_retry_allowed: true,
                credential_retry_allowed: true,
            },)
            .unwrap(),
            DeepSeekAttemptAction::Return
        );
    }

    #[test]
    fn error_attempts_do_not_consume_first_event_budget() {
        assert_eq!(
            attempt_action(DeepSeekAttemptInput {
                kind: DeepSeekAttemptKind::Error,
                attempted_first_event_retries: 1,
                first_event_committed: false,
                model_index: 0,
                model_count: 2,
                credential_index: 0,
                credential_count: 2,
                model_retry_allowed: true,
                credential_retry_allowed: true,
            })
            .unwrap(),
            DeepSeekAttemptAction::NextModel
        );
        assert_eq!(
            attempt_action(DeepSeekAttemptInput {
                kind: DeepSeekAttemptKind::Error,
                attempted_first_event_retries: 0,
                first_event_committed: true,
                model_index: 1,
                model_count: 2,
                credential_index: 1,
                credential_count: 2,
                model_retry_allowed: true,
                credential_retry_allowed: true,
            })
            .unwrap(),
            DeepSeekAttemptAction::Return
        );
    }

    #[test]
    fn invalid_shape_and_abi_fail_closed() {
        assert_eq!(
            unsafe { prodex_deepseek_first_event_retry_allowed_v1(0, 0, 0) },
            -4
        );
        assert_eq!(
            unsafe { prodex_deepseek_attempt_action_v1(ABI_VERSION, 0, 0, 0, 2, 2, 0, 1, 1, 1) },
            -1
        );
    }
}
