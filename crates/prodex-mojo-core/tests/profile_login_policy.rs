#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::profile_login_policy::{
    AuthCommitPlan, AutoLoginRoute, LoginExecution, LoginTargetValidation, ProviderLoginValidation,
    auth_commit_plan, auto_login_route, login_execution, validate_login_target,
    validate_provider_login,
};

#[test]
fn profile_login_policy_is_real_mojo_at_the_rust_boundary() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        validate_provider_login("openai", 0).unwrap(),
        ProviderLoginValidation::Allowed
    );
    assert_eq!(
        validate_provider_login("copilot", 0).unwrap(),
        ProviderLoginValidation::CodexProviderUnsupported
    );
    assert_eq!(
        validate_provider_login("agy", 4).unwrap(),
        ProviderLoginValidation::ClaudeProviderUnsupported
    );
    assert_eq!(
        validate_provider_login("kiro", 4).unwrap(),
        ProviderLoginValidation::Allowed
    );

    assert_eq!(
        validate_login_target(false, false).unwrap(),
        LoginTargetValidation::Missing
    );
    assert_eq!(
        validate_login_target(true, false).unwrap(),
        LoginTargetValidation::Changed
    );
    assert_eq!(
        validate_login_target(true, true).unwrap(),
        LoginTargetValidation::Valid
    );

    assert_eq!(
        login_execution(6).unwrap(),
        LoginExecution::DirectProfileHome
    );
    assert_eq!(
        login_execution(2).unwrap(),
        LoginExecution::TemporaryLoginHome
    );
    assert_eq!(
        auth_commit_plan("api-key", true).unwrap(),
        AuthCommitPlan {
            is_api_key: true,
            clear_email: true,
            write_base_url: true,
        }
    );
    assert_eq!(
        auth_commit_plan("chatgpt", true).unwrap(),
        AuthCommitPlan {
            is_api_key: false,
            clear_email: false,
            write_base_url: false,
        }
    );
    assert_eq!(
        auto_login_route(0, None).unwrap(),
        AutoLoginRoute::AuthLabelRequired
    );
    assert_eq!(
        auto_login_route(2, Some("api-key")).unwrap(),
        AutoLoginRoute::ApiKey
    );
}
