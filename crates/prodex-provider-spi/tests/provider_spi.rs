use prodex_provider_core::ProviderErrorClass;
use prodex_provider_spi::{
    ProviderRetryCause, ProviderRetryDecision, ProviderRetryPolicy, ProviderRetryStage,
    RuntimeProviderBindingIdentity, plan_provider_retry,
};

#[test]
fn precommit_retry_allows_retryable_provider_failure_with_budget() {
    let plan = plan_provider_retry(
        ProviderRetryPolicy::single_retry(),
        ProviderRetryStage::BeforeFirstByte,
        ProviderRetryCause::NextProvider,
        ProviderErrorClass::Transient,
        0,
    );
    assert_eq!(plan.decision, ProviderRetryDecision::Allowed);
    assert_eq!(plan.remaining_precommit_retries, 1);
}

#[test]
fn retry_never_replays_after_commit_or_cancellation() {
    for stage in [
        ProviderRetryStage::AfterFirstByte,
        ProviderRetryStage::AfterCancellation,
    ] {
        let plan = plan_provider_retry(
            ProviderRetryPolicy::bounded(4),
            stage,
            ProviderRetryCause::NextProvider,
            ProviderErrorClass::Transient,
            0,
        );
        assert_eq!(plan.decision, ProviderRetryDecision::DeniedCommitted);
    }
}

#[test]
fn retry_budget_is_bounded() {
    let plan = plan_provider_retry(
        ProviderRetryPolicy::single_retry(),
        ProviderRetryStage::BeforeDispatch,
        ProviderRetryCause::RotateCredential,
        ProviderErrorClass::Auth,
        1,
    );
    assert_eq!(plan.decision, ProviderRetryDecision::DeniedBudgetExhausted);
    assert_eq!(plan.remaining_precommit_retries, 0);
}

#[test]
fn cause_and_error_class_must_be_compatible() {
    let plan = plan_provider_retry(
        ProviderRetryPolicy::bounded(3),
        ProviderRetryStage::BeforeDispatch,
        ProviderRetryCause::NextProvider,
        ProviderErrorClass::Auth,
        0,
    );
    assert_eq!(plan.decision, ProviderRetryDecision::DeniedNotRetryable);

    let model = plan_provider_retry(
        ProviderRetryPolicy::bounded(3),
        ProviderRetryStage::BeforeDispatch,
        ProviderRetryCause::NextModel,
        ProviderErrorClass::NotFound,
        0,
    );
    assert_eq!(model.decision, ProviderRetryDecision::Allowed);
}

#[test]
fn provider_spi_binding_identity_uses_the_provider_core_policy_path() {
    let identity = RuntimeProviderBindingIdentity::from_profile(
        prodex_provider_core::ProviderId::Anthropic,
        "synthetic-profile",
        "https://api.example.com/v1/",
    )
    .expect("valid provider binding identity");
    assert_eq!(
        identity.provider(),
        prodex_provider_core::ProviderId::Anthropic
    );
    assert_eq!(
        identity,
        RuntimeProviderBindingIdentity::from_profile(
            prodex_provider_core::ProviderId::Anthropic,
            "synthetic-profile",
            "https://api.example.com/v1",
        )
        .unwrap()
    );
    assert!(
        RuntimeProviderBindingIdentity::from_profile(
            prodex_provider_core::ProviderId::Anthropic,
            "bad\u{0000}profile",
            "https://api.example.com/v1",
        )
        .is_none()
    );
}
