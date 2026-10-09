#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::kiro_import_policy::{
    KiroAuthKeySource, KiroAuthKind, KiroAuthStoreAction, KiroImportAction, KiroImportActionError,
    kiro_auth_key_source, kiro_auth_kind, kiro_auth_store_action, kiro_import_action,
    kiro_profile_match,
};

#[test]
fn kiro_import_policy_required_mojo_matrix_covers_conflicts_and_empty_inputs() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    for (presence, expected) in [
        ([true, true, true], Some(KiroAuthKeySource::Social)),
        ([false, true, true], Some(KiroAuthKeySource::ExternalIdp)),
        ([false, false, true], Some(KiroAuthKeySource::OtherPriority)),
        ([false, false, false], None),
    ] {
        assert_eq!(kiro_auth_key_source(presence).unwrap(), expected);
    }

    assert_eq!(
        kiro_auth_kind(Some(KiroAuthKeySource::Social), true, false).unwrap(),
        KiroAuthKind::Social
    );
    assert_eq!(
        kiro_auth_kind(Some(KiroAuthKeySource::ExternalIdp), true, true).unwrap(),
        KiroAuthKind::ExternalIdp
    );
    assert_eq!(
        kiro_auth_kind(None, true, false).unwrap(),
        KiroAuthKind::IdentityCenter
    );
    assert_eq!(
        kiro_auth_kind(None, false, false).unwrap(),
        KiroAuthKind::BuilderId
    );

    assert_eq!(kiro_profile_match(&[0, 3, 7, 7]).unwrap(), Some(2));
    assert_eq!(kiro_profile_match(&[]).unwrap(), None);
    assert_eq!(
        kiro_profile_match(&[8]),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
    assert_eq!(
        kiro_profile_match(&[0; 65_537]),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}

#[test]
fn kiro_import_policy_required_mojo_matrix_covers_create_update_and_validation() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    assert_eq!(
        kiro_import_action(false, false, true, true).unwrap(),
        KiroImportAction::Create
    );
    assert_eq!(
        kiro_import_action(false, true, true, false).unwrap(),
        KiroImportAction::Create
    );
    assert_eq!(
        kiro_import_action(true, false, true, true).unwrap(),
        KiroImportAction::Update
    );
    assert_eq!(
        kiro_import_action(true, true, true, true).unwrap(),
        KiroImportAction::Update
    );
    assert_eq!(
        kiro_import_action(false, true, false, false),
        Err(KiroImportActionError::InvalidRequestedName)
    );
    assert_eq!(
        kiro_import_action(true, true, true, false),
        Err(KiroImportActionError::ExistingNameMismatch)
    );

    for (input, expected) in [
        (
            (false, false, false, false),
            KiroAuthStoreAction::UseIncoming,
        ),
        ((true, true, true, true), KiroAuthStoreAction::UseIncoming),
        ((true, true, false, true), KiroAuthStoreAction::UseIncoming),
        ((false, true, false, false), KiroAuthStoreAction::UseStored),
    ] {
        assert_eq!(
            kiro_auth_store_action(input.0, input.1, input.2, input.3).unwrap(),
            expected
        );
    }
    assert_eq!(
        kiro_auth_store_action(false, true, true, true),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}
