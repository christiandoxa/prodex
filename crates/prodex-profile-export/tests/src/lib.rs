use super::*;
use zeroize::Zeroize;

mod envelope;
mod journal;

use envelope::profile_export_private_temp_dir;

fn plan_profile(
    name: &str,
    supports_codex_runtime: bool,
    email: Option<&str>,
    account_id: Option<&str>,
) -> ProfileImportPlanInput {
    ProfileImportPlanInput {
        profile_name: name.to_string(),
        supports_codex_runtime,
        identity: ProfileImportIdentity {
            email: email.map(ToOwned::to_owned),
            account_id: account_id.map(ToOwned::to_owned),
        },
    }
}

fn available(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn requested_profile_names_default_to_available_names() {
    let names = resolve_requested_profile_names(&available(&["second", "main"]), &[])
        .expect("profile names should resolve");

    assert_eq!(names, vec!["main".to_string(), "second".to_string()]);
}

#[test]
fn requested_profile_names_deduplicate_and_preserve_request_order() {
    let requested = vec![
        "second".to_string(),
        "main".to_string(),
        "second".to_string(),
    ];
    let names = resolve_requested_profile_names(&available(&["main", "second"]), &requested)
        .expect("profile names should resolve");

    assert_eq!(names, vec!["second".to_string(), "main".to_string()]);
}

#[test]
fn requested_profile_names_reject_missing_profiles() {
    let requested = vec!["missing".to_string()];
    let err = resolve_requested_profile_names(&available(&["main"]), &requested)
        .expect_err("missing profile should fail");

    assert_eq!(err.to_string(), "profile 'missing' does not exist");
}

#[test]
fn export_summary_fields_match_cli_panel_fields() {
    let fields = profile_export_summary_fields(ProfileExportSummary {
        profile_count: 2,
        path: "/tmp/profiles.json".to_string(),
        encrypted: true,
        active_profile: Some("main".to_string()),
    });

    assert_eq!(
        fields,
        vec![
            ("Result".to_string(), "Exported 2 profile(s).".to_string()),
            ("Path".to_string(), "/tmp/profiles.json".to_string()),
            ("Encrypted".to_string(), "Yes".to_string()),
            ("Active".to_string(), "main".to_string()),
        ]
    );
}

#[test]
fn import_summary_fields_match_cli_panel_fields() {
    let fields = profile_import_summary_fields(ProfileImportSummary {
        imported_count: 2,
        updated_existing_count: 1,
        path: "/tmp/profiles.json".to_string(),
        encrypted: false,
        source_active_profile: Some("source".to_string()),
        active_profile: Some("target".to_string()),
    });

    assert_eq!(
        fields,
        vec![
            (
                "Result".to_string(),
                "Imported 2 profile(s) and updated 1 existing profile(s).".to_string(),
            ),
            ("Path".to_string(), "/tmp/profiles.json".to_string()),
            ("Encrypted".to_string(), "No".to_string()),
            ("Imported".to_string(), "2".to_string()),
            ("Updated duplicates".to_string(), "1".to_string()),
            ("Source active".to_string(), "source".to_string()),
            ("Active".to_string(), "target".to_string()),
        ]
    );
}

#[test]
fn imported_active_profile_uses_existing_active_profile_first() {
    let resolved = BTreeMap::from([("source".to_string(), "target".to_string())]);

    assert_eq!(
        resolve_imported_active_profile(Some("current"), Some("source"), &resolved),
        Some("current".to_string())
    );
    assert_eq!(
        resolve_imported_active_profile(None, Some("source"), &resolved),
        Some("target".to_string())
    );
    assert_eq!(
        resolve_imported_active_profile(None, Some("missing"), &resolved),
        None
    );
}

#[test]
fn export_active_profile_only_survives_when_selected() {
    assert_eq!(
        resolve_profile_export_active_profile(Some("main"), ["main", "second"]),
        Some("main".to_string())
    );
    assert_eq!(
        resolve_profile_export_active_profile(Some("other"), ["main", "second"]),
        None
    );
    assert_eq!(
        resolve_profile_export_active_profile(None, ["main", "second"]),
        None
    );
}

#[test]
fn import_transaction_records_commit_order_and_previous_active() {
    let mut transaction = ImportedProfilesTransaction::new(Some("previous".to_string()), 2, 1);

    transaction.record_imported_profile("main".to_string(), PathBuf::from("/tmp/main"));
    transaction.record_existing_auth_update(ImportedExistingProfileAuthUpdate {
        profile_name: "existing".to_string(),
        codex_home: PathBuf::from("/tmp/existing"),
        previous_auth_json: Some("old-auth".to_string()),
        previous_email: Some("old@example.com".to_string()),
        journal_path: Some(PathBuf::from("/tmp/journal.json")),
        restore_auth_json: true,
        previous_provider_json: None,
        previous_secret_files: Vec::new(),
    });

    let commit = transaction.into_commit();

    assert_eq!(commit.imported_names, vec!["main".to_string()]);
    assert_eq!(commit.updated_existing_names, vec!["existing".to_string()]);
    assert_eq!(commit.committed_homes, vec![PathBuf::from("/tmp/main")]);
    assert_eq!(commit.previous_active_profile.as_deref(), Some("previous"));
    assert_eq!(commit.auth_updates[0].profile_name, "existing");
}

#[test]
fn import_plan_updates_same_name_runtime_profile() {
    let profiles = [plan_profile(
        "main",
        true,
        Some("user@example.com"),
        Some("acct-main"),
    )];

    let plan = plan_profile_import(
        &profiles,
        |name| (name == "main").then_some(true),
        |_| Ok(None),
    )
    .expect("plan should resolve");

    assert_eq!(
        plan.actions,
        vec![ProfileImportPlanAction::UpdateExisting {
            source_index: 0,
            target_profile_name: "main".to_string(),
        }]
    );
    assert_eq!(
        plan.resolved_profile_names,
        BTreeMap::from([("main".to_string(), "main".to_string())])
    );
}

#[test]
fn import_plan_updates_same_name_non_runtime_profile() {
    let profiles = [plan_profile(
        "gemini-main",
        false,
        Some("gemini@example.com"),
        None,
    )];

    let plan = plan_profile_import(
        &profiles,
        |name| (name == "gemini-main").then_some(false),
        |_| Ok(None),
    )
    .expect("plan should update same-name provider profile");

    assert_eq!(
        plan.actions,
        vec![ProfileImportPlanAction::UpdateExisting {
            source_index: 0,
            target_profile_name: "gemini-main".to_string(),
        }]
    );
    assert_eq!(
        plan.resolved_profile_names,
        BTreeMap::from([("gemini-main".to_string(), "gemini-main".to_string())])
    );
}

#[test]
fn import_plan_rejects_same_name_runtime_provider_mismatch() {
    let profiles = [plan_profile(
        "main",
        true,
        Some("main@example.com"),
        Some("acct-main"),
    )];

    let error = plan_profile_import(
        &profiles,
        |name| (name == "main").then_some(false),
        |_| Ok(None),
    )
    .expect_err("cross-provider import must be rejected");

    assert!(error.to_string().contains("incompatible provider"));
}

#[test]
fn import_plan_rewrites_pending_new_profile_for_duplicate_identity() {
    let profiles = [
        plan_profile("first", true, Some("user@example.com"), Some("acct-main")),
        plan_profile("second", true, Some("User@Example.com"), Some("acct-main")),
    ];

    let plan = plan_profile_import(&profiles, |_| None, |_| Ok(None)).expect("plan should resolve");

    assert_eq!(
        plan.actions,
        vec![
            ProfileImportPlanAction::StageNew {
                source_index: 0,
                staged_index: 0,
            },
            ProfileImportPlanAction::RewriteStagedAuth {
                source_index: 1,
                staged_index: 0,
            },
        ]
    );
    assert_eq!(
        plan.resolved_profile_names,
        BTreeMap::from([
            ("first".to_string(), "first".to_string()),
            ("second".to_string(), "first".to_string()),
        ])
    );
}

#[test]
fn import_plan_keeps_same_account_with_different_emails_distinct() {
    let profiles = [
        plan_profile("first", true, Some("first@example.com"), Some("acct-main")),
        plan_profile(
            "second",
            true,
            Some("second@example.com"),
            Some("acct-main"),
        ),
    ];

    let plan = plan_profile_import(&profiles, |_| None, |_| Ok(None)).expect("plan should resolve");

    assert_eq!(
        plan.actions,
        vec![
            ProfileImportPlanAction::StageNew {
                source_index: 0,
                staged_index: 0,
            },
            ProfileImportPlanAction::StageNew {
                source_index: 1,
                staged_index: 1,
            },
        ]
    );
    assert_eq!(
        plan.resolved_profile_names,
        BTreeMap::from([
            ("first".to_string(), "first".to_string()),
            ("second".to_string(), "second".to_string()),
        ])
    );
}

#[test]
fn import_plan_uses_external_identity_match() {
    let profiles = [plan_profile(
        "incoming",
        true,
        Some("user@example.com"),
        Some("acct-main"),
    )];

    let plan = plan_profile_import(&profiles, |_| None, |_| Ok(Some("existing".to_string())))
        .expect("plan should resolve");

    assert_eq!(
        plan.actions,
        vec![ProfileImportPlanAction::UpdateExisting {
            source_index: 0,
            target_profile_name: "existing".to_string(),
        }]
    );
    assert_eq!(
        plan.resolved_profile_names,
        BTreeMap::from([("incoming".to_string(), "existing".to_string())])
    );
}

#[test]
fn import_plan_reuses_external_identity_match_without_a_second_lookup() {
    let profiles = [
        plan_profile(
            "incoming-one",
            true,
            Some("User@example.com"),
            Some("acct-main"),
        ),
        plan_profile(
            "incoming-two",
            true,
            Some("user@example.com"),
            Some("acct-main"),
        ),
    ];
    let mut looked_up = Vec::new();

    let plan = plan_profile_import(
        &profiles,
        |_| None,
        |identity| {
            looked_up.push(identity.clone());
            Ok(Some("existing".to_string()))
        },
    )
    .expect("matching identity should resolve");

    assert_eq!(
        looked_up,
        vec![ProfileImportIdentity {
            email: Some("User@example.com".to_string()),
            account_id: Some("acct-main".to_string()),
        }]
    );
    assert_eq!(
        plan.actions,
        vec![
            ProfileImportPlanAction::UpdateExisting {
                source_index: 0,
                target_profile_name: "existing".to_string(),
            },
            ProfileImportPlanAction::UpdateExisting {
                source_index: 1,
                target_profile_name: "existing".to_string(),
            },
        ]
    );
    assert_eq!(
        plan.resolved_profile_names,
        BTreeMap::from([
            ("incoming-one".to_string(), "existing".to_string()),
            ("incoming-two".to_string(), "existing".to_string()),
        ])
    );
}

#[test]
fn import_plan_rejects_empty_bundle() {
    let error = plan_profile_import(&[], |_| None, |_| Ok(None))
        .expect_err("empty profile bundles should fail");

    assert_eq!(
        error.to_string(),
        "profile export bundle does not contain any profiles"
    );
}

#[test]
fn import_source_name_validation_rejects_duplicates() {
    let err = validate_profile_import_source_names(["main", "backup", "main"])
        .expect_err("duplicate profile names should fail");

    assert_eq!(
        err.to_string(),
        "profile export bundle contains duplicate profile 'main'"
    );
}

#[test]
fn import_identity_falls_back_to_exported_email() {
    assert_eq!(
        resolve_profile_import_identity(
            ProfileImportIdentity {
                email: None,
                account_id: Some("acct-main".to_string()),
            },
            Some("\u{2003} User@Example.com \u{3000}"),
        ),
        ProfileImportIdentity {
            email: Some("User@Example.com".to_string()),
            account_id: Some("acct-main".to_string()),
        }
    );
    assert_eq!(
        resolve_profile_import_identity(
            ProfileImportIdentity {
                email: Some("auth@example.com".to_string()),
                account_id: None,
            },
            Some("fallback@example.com"),
        )
        .email
        .as_deref(),
        Some("auth@example.com")
    );
    assert_eq!(
        resolve_profile_import_identity(
            ProfileImportIdentity {
                email: None,
                account_id: None,
            },
            Some(" \u{2003} "),
        )
        .email,
        None
    );
    assert_eq!(
        resolve_profile_import_identity(
            ProfileImportIdentity {
                email: Some(String::new()),
                account_id: None,
            },
            Some("fallback@example.com"),
        )
        .email,
        Some(String::new())
    );
}

#[test]
fn import_identity_key_combines_account_and_email_when_both_exist() {
    assert_eq!(
        profile_import_identity_parts_target_key(Some(" acct-main "), Some(" User@Example.COM ")),
        Some("account:acct-main|email:user@example.com".to_string())
    );
    assert_eq!(
        profile_import_identity_parts_target_key(Some(" acct-main "), None),
        Some("account:acct-main".to_string())
    );
    assert_eq!(
        profile_import_identity_parts_target_key(None, Some(" User@Example.COM ")),
        Some("email:user@example.com".to_string())
    );
    assert_ne!(
        profile_import_identity_parts_target_key(Some("acct-main"), Some("first@example.com")),
        profile_import_identity_parts_target_key(Some("acct-main"), Some("second@example.com"))
    );
}

#[test]
fn queued_auth_update_keeps_last_token_and_non_empty_email() {
    let mut updates = Vec::new();

    queue_profile_import_auth_update(
        &mut updates,
        "main",
        Some("first@example.com".to_string()),
        "first-auth".to_string(),
    );
    queue_profile_import_auth_update(&mut updates, "main", None, "second-auth".to_string());

    assert_eq!(
        updates,
        vec![ProfileImportAuthUpdatePlan {
            target_profile_name: "main".to_string(),
            email: Some("first@example.com".to_string()),
            auth_json: "second-auth".to_string(),
        }]
    );
}

#[test]
fn queued_auth_update_uses_mojo_selected_first_matching_target() {
    let mut updates = vec![
        ProfileImportAuthUpdatePlan {
            target_profile_name: "other".to_string(),
            email: None,
            auth_json: "other-auth".to_string(),
        },
        ProfileImportAuthUpdatePlan {
            target_profile_name: "main".to_string(),
            email: Some("first@example.com".to_string()),
            auth_json: "first-auth".to_string(),
        },
        ProfileImportAuthUpdatePlan {
            target_profile_name: "main".to_string(),
            email: Some("second@example.com".to_string()),
            auth_json: "second-auth".to_string(),
        },
    ];

    queue_profile_import_auth_update(&mut updates, "main", None, "replacement-auth".to_string());

    assert_eq!(updates[1].auth_json, "replacement-auth");
    assert_eq!(updates[1].email.as_deref(), Some("first@example.com"));
    assert_eq!(updates[2].auth_json, "second-auth");
    assert_eq!(updates[2].email.as_deref(), Some("second@example.com"));
}

#[path = "copilot.rs"]
mod copilot;
