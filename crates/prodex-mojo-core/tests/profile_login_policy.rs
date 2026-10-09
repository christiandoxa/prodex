#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::profile_login_policy::{
    AuthCommitPlan, AutoLoginRoute, AutoLoginTransition, LoginCandidateSelection, LoginExecution,
    LoginMethodPlan, LoginMethodRoute, LoginTargetValidation, ProviderLoginValidation,
    auth_commit_plan, auto_login_route, auto_login_transition, login_execution, login_method_plan,
    select_login_candidate, validate_login_target, validate_provider_login,
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

#[test]
fn profile_login_transition_matrix_keeps_precedence_and_existing_state() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    for existing in [false, true] {
        assert_eq!(
            auto_login_transition(6, Some("api-key"), existing).unwrap(),
            AutoLoginTransition::Status
        );
        assert_eq!(
            auto_login_transition(4, Some("api-key"), existing).unwrap(),
            AutoLoginTransition::Anthropic
        );
        assert_eq!(
            auto_login_transition(0, None, existing).unwrap(),
            AutoLoginTransition::AuthLabelRequired
        );
        assert_eq!(
            auto_login_transition(0, Some("api-key"), existing).unwrap(),
            if existing {
                AutoLoginTransition::ApiKeyExisting
            } else {
                AutoLoginTransition::ApiKeyNew
            }
        );
        assert_eq!(
            auto_login_transition(0, Some("chatgpt"), existing).unwrap(),
            if existing {
                AutoLoginTransition::IdentityExisting
            } else {
                AutoLoginTransition::IdentityNew
            }
        );
    }

    assert!(auto_login_transition(-1, None, false).is_err());
    assert!(auto_login_transition(7, None, false).is_err());
}

#[test]
fn profile_login_candidate_order_and_duplicate_count_are_mojo_owned() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        select_login_candidate(&[false, true, true, false]).unwrap(),
        LoginCandidateSelection {
            first_match: Some(1),
            match_count: 2,
        }
    );
    assert_eq!(
        select_login_candidate(&[true, false, true]).unwrap(),
        LoginCandidateSelection {
            first_match: Some(0),
            match_count: 2,
        }
    );
    assert_eq!(
        select_login_candidate(&[]).unwrap(),
        LoginCandidateSelection {
            first_match: None,
            match_count: 0,
        }
    );
}

#[test]
fn profile_login_method_and_auth_outcomes_keep_precedence_in_mojo() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        login_method_plan(2, true).unwrap(),
        LoginMethodPlan {
            route: LoginMethodRoute::DirectApiKey,
            allows_base_url: true,
        }
    );
    assert_eq!(
        login_method_plan(2, false).unwrap(),
        LoginMethodPlan {
            route: LoginMethodRoute::CodexChild,
            allows_base_url: true,
        }
    );
    assert_eq!(
        login_method_plan(4, true).unwrap(),
        LoginMethodPlan {
            route: LoginMethodRoute::ExternalClaude,
            allows_base_url: false,
        }
    );
    assert_eq!(
        auth_commit_plan("api-key", false).unwrap(),
        AuthCommitPlan {
            is_api_key: true,
            clear_email: true,
            write_base_url: false,
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
}

#[test]
fn profile_login_provider_validation_rejects_missing_and_invalid_combinations() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert!(validate_provider_login("", 0).is_err());
    assert!(validate_provider_login("unknown", 0).is_err());
    assert!(validate_provider_login("openai", -1).is_err());
    assert!(
        validate_provider_login("anthropic", 0).is_ok_and(|decision| {
            decision == ProviderLoginValidation::CodexProviderUnsupported
        })
    );
    assert!(
        validate_provider_login("anthropic", 4)
            .is_ok_and(|decision| { decision == ProviderLoginValidation::Allowed })
    );
}
