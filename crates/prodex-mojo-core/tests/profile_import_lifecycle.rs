#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::profile_export::{
    ProfileImportHomeAction, ProfileImportHomeActionKind, ProfileImportProviderTransition,
    ProfileImportRecoveryAction, profile_import_home_action, profile_import_journal_order,
    profile_import_lifecycle_order, profile_import_provider_transition,
    profile_import_recovery_action,
};

#[test]
fn profile_import_lifecycle_abi_is_real_mojo_and_bounded() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        profile_import_lifecycle_order(&["重复", "alpha", "重复"])
            .unwrap()
            .profile_indices,
        vec![1, 0]
    );
    assert_eq!(
        profile_import_journal_order(
            &["2026-10-10T00:00:00Z", "2026-10-10T00:00:01Z"],
            &["a", "b"],
        )
        .unwrap(),
        vec![1, 0]
    );
    assert_eq!(
        profile_import_provider_transition("openai", "openai").unwrap(),
        ProfileImportProviderTransition::Compatible
    );
    assert_eq!(
        profile_import_provider_transition("openai", "anthropic").unwrap(),
        ProfileImportProviderTransition::Mismatch
    );
    assert!(profile_import_provider_transition("", "openai").is_err());
    let error = profile_import_provider_transition("bearer-secret", "openai").unwrap_err();
    assert!(!format!("{error:?}").contains("bearer-secret"));
}

#[test]
fn profile_import_recovery_precedence_and_home_actions_are_mojo_owned() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        profile_import_recovery_action(true, false, true, true).unwrap(),
        ProfileImportRecoveryAction::Skip
    );
    assert_eq!(
        profile_import_recovery_action(false, true, true, true).unwrap(),
        ProfileImportRecoveryAction::Commit
    );
    assert_eq!(
        profile_import_recovery_action(false, true, false, true).unwrap(),
        ProfileImportRecoveryAction::Rollback
    );

    assert_eq!(
        profile_import_home_action(
            ProfileImportHomeActionKind::Promote,
            true,
            true,
            false,
            false,
        )
        .unwrap(),
        ProfileImportHomeAction::Promote
    );
    assert_eq!(
        profile_import_home_action(
            ProfileImportHomeActionKind::Promote,
            false,
            false,
            true,
            false,
        )
        .unwrap(),
        ProfileImportHomeAction::RestoreSource
    );
    assert_eq!(
        profile_import_home_action(
            ProfileImportHomeActionKind::Create,
            false,
            false,
            true,
            false,
        )
        .unwrap(),
        ProfileImportHomeAction::CleanupDestination
    );
}
