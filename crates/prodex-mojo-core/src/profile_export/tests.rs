use crate as prodex_mojo_core;
use prodex_mojo_core::profile_export::*;

#[test]
fn profile_export_selection_plan_is_mojo_owned() {
    let available = ["main", "second"];
    assert_eq!(
        profile_export_selection_plan(&available, &[]).unwrap(),
        ProfileExportSelectionPlan::Selected(vec![0, 1])
    );
    assert_eq!(
        profile_export_selection_plan(&available, &["second", "main", "second"]).unwrap(),
        ProfileExportSelectionPlan::Selected(vec![1, 0])
    );
    assert_eq!(
        profile_export_selection_plan(&available, &["missing"]).unwrap(),
        ProfileExportSelectionPlan::MissingRequested(0)
    );
    assert_eq!(
        profile_export_selection_plan(&[], &["main"]).unwrap(),
        ProfileExportSelectionPlan::NoProfiles
    );
}

#[test]
fn profile_active_selection_and_import_plan_are_mojo_owned() {
    assert!(profile_export_active_profile_selected(Some("main"), &["second", "main"]).unwrap());
    assert!(!profile_export_active_profile_selected(Some("other"), &["second", "main"]).unwrap());
    assert!(!profile_export_active_profile_selected(None, &["main"]).unwrap());

    let resolved = [("source", "target"), ("other", "other-target")];
    assert_eq!(
        profile_import_active_profile_plan(Some("current"), Some("source"), &resolved).unwrap(),
        ProfileImportActiveProfilePlan::Existing,
    );
    assert_eq!(
        profile_import_active_profile_plan(None, Some("source"), &resolved).unwrap(),
        ProfileImportActiveProfilePlan::Resolved(0),
    );
    assert_eq!(
        profile_import_active_profile_plan(None, Some("missing"), &resolved).unwrap(),
        ProfileImportActiveProfilePlan::None,
    );
    assert_eq!(
        profile_import_active_profile_plan(None, None, &resolved).unwrap(),
        ProfileImportActiveProfilePlan::None,
    );
}

#[test]
fn profile_export_envelope_schema_is_owned_by_mojo_and_rejects_invalid_metadata() {
    let v2 = |kdf: &'static str,
              salt_base64: &'static str,
              nonce_base64: &'static str,
              memory_kib: u32,
              version: u32| ProfileExportEnvelopeSchemaInput::EncryptedV2 {
        format: "prodex_profile_export",
        version,
        cipher: "aes_256_gcm_siv",
        kdf,
        argon2_version: 0x13,
        argon2_memory_kib: memory_kib,
        argon2_iterations: 1,
        argon2_parallelism: 1,
        salt_base64,
        nonce_base64,
        ciphertext_base64: "AAAAAAAAAAAAAAAAAAAAAA==",
    };
    let salt = "AAAAAAAAAAAAAAAAAAAAAA==";
    let nonce = "AAAAAAAAAAAAAAAA";
    assert_eq!(
        validate_profile_export_envelope_schema(v2("argon2id", salt, nonce, 8 * 1024, 2)),
        Ok(())
    );
    assert_eq!(
        validate_profile_export_envelope_schema(v2("pbkdf2_sha256", salt, nonce, 8 * 1024, 2)),
        Err(ProfileExportEnvelopeSchemaError::Kdf),
        "legacy PBKDF2 must not be accepted as an Argon2id V2 envelope"
    );
    assert_eq!(
        validate_profile_export_envelope_schema(v2("argon2id", salt, "AA==", 8 * 1024, 2)),
        Err(ProfileExportEnvelopeSchemaError::NonceLength)
    );
    assert_eq!(
        validate_profile_export_envelope_schema(v2("argon2id", "not base64!", nonce, 8 * 1024, 2)),
        Err(ProfileExportEnvelopeSchemaError::SaltEncoding)
    );
    assert_eq!(
        validate_profile_export_envelope_schema(v2("argon2id", salt, nonce, 1, 2)),
        Err(ProfileExportEnvelopeSchemaError::Argon2Memory)
    );
    assert_eq!(
        validate_profile_export_envelope_schema(v2("argon2id", salt, nonce, 8 * 1024, 1)),
        Err(ProfileExportEnvelopeSchemaError::KindVersion)
    );
}

#[test]
fn profile_export_policy_kernel_smoke() {
    assert!(validate_collection(256, 4096).is_ok());
    assert_eq!(
        validate_collection(257, 0),
        Err(ProfileExportPolicyViolation::ProfileCount)
    );
    assert_eq!(
        validate_profile_secret_files(17),
        Err(ProfileExportPolicyViolation::ProfileSecretFileCount)
    );
    assert_eq!(
        validate_password_bytes(0),
        Err(ProfileExportPolicyViolation::PasswordSize)
    );
    assert!(validate_pbkdf2_iterations(50_000).is_ok());
    assert_eq!(validate_argon2(0x13, 8 * 1024, 1, 1), Ok(()));
    assert_eq!(
        profile_export_password_mode_action(false, false, false).unwrap(),
        ProfileExportPasswordAction::NonInteractiveError
    );
    assert_eq!(
        profile_export_password_mode_action(true, true, false).unwrap(),
        ProfileExportPasswordAction::Protect
    );
    assert_eq!(
        profile_password_source_action(true, false).unwrap(),
        ProfileExportPasswordAction::Environment
    );
    assert_eq!(
        profile_export_password_validation(false, false).unwrap(),
        ProfileExportPasswordAction::Mismatch
    );
    assert_eq!(
        profile_import_password_validation(true).unwrap(),
        ProfileExportPasswordAction::Empty
    );
    assert_eq!(
        profile_import_auth_update_action(false, false).unwrap(),
        ProfileImportAuthUpdateAction::Append
    );
    assert_eq!(
        profile_import_auth_update_action(true, false).unwrap(),
        ProfileImportAuthUpdateAction::ReplaceAuth
    );
    assert_eq!(
        profile_import_auth_update_action(true, true).unwrap(),
        ProfileImportAuthUpdateAction::ReplaceAuthAndEmail
    );
}

#[test]
fn profile_import_auth_update_plan_is_mojo_owned_and_input_sensitive() {
    let empty = profile_import_auth_update_plan("main", &[], false).unwrap();
    assert_eq!(
        empty,
        ProfileImportAuthUpdatePlan {
            action: ProfileImportAuthUpdateAction::Append,
            existing_index: None,
        }
    );

    let existing = ["other", "main", "main"];
    let without_email = profile_import_auth_update_plan("main", &existing, false).unwrap();
    assert_eq!(
        without_email,
        ProfileImportAuthUpdatePlan {
            action: ProfileImportAuthUpdateAction::ReplaceAuth,
            existing_index: Some(1),
        }
    );
    let with_email = profile_import_auth_update_plan("main", &existing, true).unwrap();
    assert_eq!(
        with_email,
        ProfileImportAuthUpdatePlan {
            action: ProfileImportAuthUpdateAction::ReplaceAuthAndEmail,
            existing_index: Some(1),
        }
    );
    assert_ne!(without_email, with_email);
    assert_ne!(
        without_email,
        profile_import_auth_update_plan("new", &existing, false).unwrap()
    );
}
