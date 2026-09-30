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
    let stage_tag = match stage {
        ProviderRetryStage::BeforeDispatch => 0,
        ProviderRetryStage::BeforeFirstByte => 1,
        ProviderRetryStage::AfterFirstByte => 2,
        ProviderRetryStage::AfterCancellation => 3,
    };
    let cause_tag = match cause {
        ProviderRetryCause::NextModel => 0,
        ProviderRetryCause::RotateCredential => 1,
        ProviderRetryCause::NextProvider => 2,
    };
    let error_class_tag = match error_class {
        ProviderErrorClass::Auth => 0,
        ProviderErrorClass::Quota => 1,
        ProviderErrorClass::RateLimit => 2,
        ProviderErrorClass::Transient => 3,
        ProviderErrorClass::NotFound => 4,
        ProviderErrorClass::Other => 5,
    };
    let plan = prodex_mojo_core::provider_constraints::provider_retry_plan(
        policy.max_precommit_attempts,
        stage_tag,
        cause_tag,
        error_class_tag,
        attempted_precommit_retries,
    )
    .expect("Mojo provider retry policy returned invalid output");
    let decision = match plan.decision {
        0 => ProviderRetryDecision::Allowed,
        1 => ProviderRetryDecision::DeniedCommitted,
        2 => ProviderRetryDecision::DeniedBudgetExhausted,
        3 => ProviderRetryDecision::DeniedNotRetryable,
        _ => unreachable!("validated Mojo provider retry decision"),
    };
    ProviderRetryPlan {
        stage,
        decision,
        attempted_precommit_retries,
        remaining_precommit_retries: plan.remaining_precommit_retries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_retry_matrix_is_mojo_authoritative() {
        let stages = [
            ProviderRetryStage::BeforeDispatch,
            ProviderRetryStage::BeforeFirstByte,
            ProviderRetryStage::AfterFirstByte,
            ProviderRetryStage::AfterCancellation,
        ];
        let causes = [
            ProviderRetryCause::NextModel,
            ProviderRetryCause::RotateCredential,
            ProviderRetryCause::NextProvider,
        ];
        let errors = [
            ProviderErrorClass::Auth,
            ProviderErrorClass::Quota,
            ProviderErrorClass::RateLimit,
            ProviderErrorClass::Transient,
            ProviderErrorClass::NotFound,
            ProviderErrorClass::Other,
        ];
        for stage in stages {
            for cause in causes {
                for error_class in errors {
                    let plan = plan_provider_retry(
                        ProviderRetryPolicy::bounded(2),
                        stage,
                        cause,
                        error_class,
                        1,
                    );
                    assert_eq!(plan.stage, stage);
                    assert_eq!(plan.attempted_precommit_retries, 1);
                    assert_eq!(plan.remaining_precommit_retries, 1);
                    if matches!(
                        stage,
                        ProviderRetryStage::AfterFirstByte | ProviderRetryStage::AfterCancellation
                    ) {
                        assert_eq!(plan.decision, ProviderRetryDecision::DeniedCommitted);
                    }
                }
            }
        }

        assert_eq!(
            plan_provider_retry(
                ProviderRetryPolicy::single_retry(),
                ProviderRetryStage::BeforeFirstByte,
                ProviderRetryCause::NextModel,
                ProviderErrorClass::Other,
                0,
            )
            .decision,
            ProviderRetryDecision::DeniedNotRetryable
        );
        assert_eq!(
            plan_provider_retry(
                ProviderRetryPolicy::single_retry(),
                ProviderRetryStage::BeforeFirstByte,
                ProviderRetryCause::NextModel,
                ProviderErrorClass::Quota,
                1,
            )
            .decision,
            ProviderRetryDecision::DeniedBudgetExhausted
        );
    }
}
