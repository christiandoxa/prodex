#![forbid(unsafe_code)]
//! Minimal transport-neutral provider runtime primitives.

use prodex_provider_core::ProviderErrorClass;
pub use prodex_provider_core::RuntimeProviderBindingIdentity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStreamMode {
    Unary,
    Streaming,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryStage {
    BeforeDispatch,
    BeforeFirstByte,
    AfterFirstByte,
    AfterCancellation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryCause {
    NextModel,
    RotateCredential,
    NextProvider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryDecision {
    Allowed,
    DeniedCommitted,
    DeniedBudgetExhausted,
    DeniedNotRetryable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderRetryPolicy {
    pub max_precommit_attempts: u8,
}

impl ProviderRetryPolicy {
    pub const fn single_retry() -> Self {
        Self {
            max_precommit_attempts: 1,
        }
    }

    pub const fn bounded(max_precommit_attempts: u8) -> Self {
        Self {
            max_precommit_attempts,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderRetryPlan {
    pub stage: ProviderRetryStage,
    pub decision: ProviderRetryDecision,
    pub attempted_precommit_retries: u8,
    pub remaining_precommit_retries: u8,
}

pub fn plan_provider_retry(
    policy: ProviderRetryPolicy,
    stage: ProviderRetryStage,
    cause: ProviderRetryCause,
    error_class: ProviderErrorClass,
    attempted_precommit_retries: u8,
) -> ProviderRetryPlan {
    let remaining_precommit_retries = policy
        .max_precommit_attempts
        .saturating_sub(attempted_precommit_retries);
    let decision = match stage {
        ProviderRetryStage::AfterFirstByte | ProviderRetryStage::AfterCancellation => {
            ProviderRetryDecision::DeniedCommitted
        }
        ProviderRetryStage::BeforeDispatch | ProviderRetryStage::BeforeFirstByte
            if !retry_eligible(cause, error_class) =>
        {
            ProviderRetryDecision::DeniedNotRetryable
        }
        ProviderRetryStage::BeforeDispatch | ProviderRetryStage::BeforeFirstByte
            if remaining_precommit_retries == 0 =>
        {
            ProviderRetryDecision::DeniedBudgetExhausted
        }
        ProviderRetryStage::BeforeDispatch | ProviderRetryStage::BeforeFirstByte => {
            ProviderRetryDecision::Allowed
        }
    };
    ProviderRetryPlan {
        stage,
        decision,
        attempted_precommit_retries,
        remaining_precommit_retries,
    }
}

fn retry_eligible(cause: ProviderRetryCause, error_class: ProviderErrorClass) -> bool {
    match cause {
        ProviderRetryCause::NextModel => matches!(
            error_class,
            ProviderErrorClass::Quota
                | ProviderErrorClass::RateLimit
                | ProviderErrorClass::Transient
                | ProviderErrorClass::NotFound
        ),
        ProviderRetryCause::RotateCredential => matches!(
            error_class,
            ProviderErrorClass::Auth
                | ProviderErrorClass::Quota
                | ProviderErrorClass::RateLimit
                | ProviderErrorClass::Transient
        ),
        ProviderRetryCause::NextProvider => matches!(
            error_class,
            ProviderErrorClass::Quota
                | ProviderErrorClass::RateLimit
                | ProviderErrorClass::Transient
        ),
    }
}
