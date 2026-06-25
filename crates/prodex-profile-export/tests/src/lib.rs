use super::*;

#[derive(Debug)]
struct PlanProfile {
    name: &'static str,
    supports_codex_runtime: bool,
    email: Option<&'static str>,
    account_id: Option<&'static str>,
}

impl ProfileImportPlanProfile for PlanProfile {
    fn profile_name(&self) -> &str {
        self.name
    }

    fn supports_codex_runtime(&self) -> bool {
        self.supports_codex_runtime
    }

    fn import_identity(&self) -> ProfileImportIdentity {
        ProfileImportIdentity {
            email: self.email.map(ToOwned::to_owned),
            account_id: self.account_id.map(ToOwned::to_owned),
        }
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
fn import_auth_update_journal_constructor_sets_current_version() {
    let journal = ImportedExistingProfileAuthUpdateJournal::new(
        "main".to_string(),
        "/tmp/main".to_string(),
        Some("old@example.com".to_string()),
        Some("{}".to_string()),
        "2026-05-02T00:00:00+00:00".to_string(),
    );

    assert_eq!(journal.version, IMPORT_AUTH_UPDATE_JOURNAL_VERSION);
    validate_import_auth_update_journal_version(journal.version)
        .expect("current journal version should validate");
    assert!(validate_import_auth_update_journal_version(journal.version + 1).is_err());
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
    let profiles = [PlanProfile {
        name: "main",
        supports_codex_runtime: true,
        email: Some("user@example.com"),
        account_id: Some("acct-main"),
    }];

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
    let profiles = [PlanProfile {
        name: "gemini-main",
        supports_codex_runtime: false,
        email: Some("gemini@example.com"),
        account_id: None,
    }];

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
fn import_plan_rewrites_pending_new_profile_for_duplicate_identity() {
    let profiles = [
        PlanProfile {
            name: "first",
            supports_codex_runtime: true,
            email: Some("user@example.com"),
            account_id: Some("acct-main"),
        },
        PlanProfile {
            name: "second",
            supports_codex_runtime: true,
            email: Some("User@Example.com"),
            account_id: Some("acct-main"),
        },
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
        PlanProfile {
            name: "first",
            supports_codex_runtime: true,
            email: Some("first@example.com"),
            account_id: Some("acct-main"),
        },
        PlanProfile {
            name: "second",
            supports_codex_runtime: true,
            email: Some("second@example.com"),
            account_id: Some("acct-main"),
        },
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
    let profiles = [PlanProfile {
        name: "incoming",
        supports_codex_runtime: true,
        email: Some("user@example.com"),
        account_id: Some("acct-main"),
    }];

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
            Some(" User@Example.com "),
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
fn copilot_config_parser_treats_empty_file_as_logged_out() {
    let config = parse_copilot_config_file(" \n\t")
        .expect("blank config should parse as empty Copilot state");

    assert!(select_copilot_logged_in_user(&config).is_none());
    assert!(config.logged_in_users.is_empty());
    assert!(config.copilot_tokens.is_empty());
}

#[test]
fn copilot_config_parser_accepts_copilot_jsonc_comments() {
    let config = parse_copilot_config_file(
        r#"// User settings belong in settings.json.
// This file is managed by GitHub Copilot CLI.
{
  "lastLoggedInUser": {"host": "https://github.com", "login": "main"},
  "loggedInUsers": [
    {"host": "https://github.com", "login": "fallback"}
  ],
  "trustedFolders": ["https://example.test//literal-in-string"]
}"#,
    )
    .expect("Copilot JSONC config should parse");

    let user = select_copilot_logged_in_user(&config).expect("user should resolve");
    assert_eq!(user.login, "main");
}

#[test]
fn copilot_config_parser_keeps_invalid_non_empty_errors() {
    let err = parse_copilot_config_file("not json")
        .expect_err("non-empty invalid config should still fail");

    assert!(
        err.to_string().contains("failed to parse Copilot config"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn copilot_config_helpers_select_user_and_token() {
    let config = parse_copilot_config_file(
        r#"{
                "loggedInUsers": [{"host": "https://github.com", "login": "fallback"}],
                "lastLoggedInUser": {"host": "https://ghe.example", "login": "main"},
                "copilotTokens": {
                    "https://ghe.example:main": " token-value "
                }
            }"#,
    )
    .expect("config should parse");

    let user = select_copilot_logged_in_user(&config).expect("user should resolve");

    assert_eq!(user.host, "https://ghe.example");
    assert_eq!(user.login, "main");
    assert_eq!(
        copilot_token_from_config(&config, "https://ghe.example", "main").as_deref(),
        Some("token-value")
    );
    assert_eq!(
        copilot_account_key(" https://ghe.example ", " main "),
        "https://ghe.example:main"
    );
}

#[test]
fn copilot_url_helpers_match_import_expectations() {
    assert_eq!(parse_copilot_version("1.2.3-beta"), (1, 2, 3));
    assert_eq!(copilot_platform_label_for("linux", "x86_64"), "linux-x64");
    assert_eq!(
        copilot_user_api_origin("github.com").unwrap(),
        "https://api.github.com"
    );
    assert_eq!(
        copilot_user_api_origin("http://127.0.0.1:1234/path").unwrap(),
        "http://127.0.0.1:1234"
    );
    assert_eq!(
        default_copilot_models_api_url("github.com"),
        "https://api.githubcopilot.com"
    );
    assert_eq!(
        default_copilot_models_api_url("https://enterprise.ghe.com"),
        "https://copilot-api.enterprise.ghe.com"
    );
}

#[test]
fn copilot_user_info_response_parses_metadata() {
    let info = parse_copilot_user_info_response(
        br#"{
                "login": "copilot-user",
                "access_type_sku": "copilot_standalone_seat_quota",
                "copilot_plan": "business",
                "endpoints": { "api": "https://api.example.githubcopilot.test" }
            }"#,
        "https://api.github.com/copilot_internal/user",
    )
    .expect("Copilot response should parse");

    assert_eq!(info.login.as_deref(), Some("copilot-user"));
    assert_eq!(
        info.access_type_sku.as_deref(),
        Some("copilot_standalone_seat_quota")
    );
    assert_eq!(
        info.endpoints
            .and_then(|endpoints| endpoints.api)
            .as_deref(),
        Some("https://api.example.githubcopilot.test")
    );
}

#[test]
fn copilot_import_plan_uses_user_info_with_api_fallback() {
    let plan = plan_copilot_profile_import(
        "https://github.example.test",
        "config-login",
        &CopilotUserInfo {
            login: Some("api-login".to_string()),
            access_type_sku: Some("sku".to_string()),
            copilot_plan: Some("business".to_string()),
            endpoints: Some(CopilotUserEndpoints {
                api: Some("https://api.github.example.test".to_string()),
            }),
            limited_user_quotas: BTreeMap::new(),
            monthly_quotas: BTreeMap::new(),
            limited_user_reset_date: None,
        },
    );

    assert_eq!(
        plan,
        CopilotProfileImportPlan {
            host: "https://github.example.test".to_string(),
            login: "api-login".to_string(),
            api_url: "https://api.github.example.test".to_string(),
            access_type_sku: Some("sku".to_string()),
            copilot_plan: Some("business".to_string()),
        }
    );

    let fallback_plan = plan_copilot_profile_import(
        "https://github.enterprise.test",
        "config-login",
        &CopilotUserInfo {
            login: None,
            access_type_sku: None,
            copilot_plan: None,
            endpoints: None,
            limited_user_quotas: BTreeMap::new(),
            monthly_quotas: BTreeMap::new(),
            limited_user_reset_date: None,
        },
    );

    assert_eq!(fallback_plan.login, "config-login");
    assert_eq!(fallback_plan.api_url, "https://api.github.enterprise.test");
}

#[test]
fn copilot_state_plan_updates_existing_or_adds_new_profile() {
    assert_eq!(
        plan_copilot_profile_import_state(
            "octo",
            Some("copilot-main"),
            Some("copilot-main"),
            true,
            false,
            |_| false,
            || "unused".to_string(),
        )
        .unwrap(),
        CopilotProfileImportStatePlan::UpdateExisting {
            profile_name: "copilot-main".to_string(),
            activate: false,
        }
    );

    assert_eq!(
        plan_copilot_profile_import_state(
            "octo",
            None,
            None,
            false,
            false,
            |_| false,
            || "copilot-octo".to_string(),
        )
        .unwrap(),
        CopilotProfileImportStatePlan::AddNew {
            profile_name: "copilot-octo".to_string(),
            activate: true,
        }
    );

    assert_eq!(
        plan_copilot_profile_import_state(
            "octo",
            Some("other"),
            Some("copilot-main"),
            true,
            true,
            |_| false,
            || "unused".to_string(),
        )
        .unwrap_err()
        .to_string(),
        "Copilot account 'octo' is already imported as profile 'copilot-main'"
    );
    assert_eq!(
        plan_copilot_profile_import_state(
            "octo",
            Some("taken"),
            None,
            true,
            false,
            |name| name == "taken",
            || "unused".to_string(),
        )
        .unwrap_err()
        .to_string(),
        "profile 'taken' already exists"
    );
}

#[test]
fn copilot_import_summary_fields_render_new_and_updated_profiles() {
    assert_eq!(
        copilot_profile_import_summary_fields(CopilotProfileImportSummary {
            profile_name: "copilot-main".to_string(),
            provider: "GitHub Copilot".to_string(),
            identity: "octo".to_string(),
            github_host: "https://github.com".to_string(),
            api_url: Some("https://api.githubcopilot.com".to_string()),
            codex_home: Some("/tmp/prodex/copilot-main".to_string()),
            active: true,
            updated_existing: false,
        }),
        vec![
            (
                "Result".to_string(),
                "Imported Copilot profile 'copilot-main'.".to_string()
            ),
            ("Profile".to_string(), "copilot-main".to_string()),
            ("Provider".to_string(), "GitHub Copilot".to_string()),
            ("Identity".to_string(), "octo".to_string()),
            ("GitHub host".to_string(), "https://github.com".to_string()),
            (
                "CODEX_HOME".to_string(),
                "/tmp/prodex/copilot-main".to_string()
            ),
            (
                "API".to_string(),
                "https://api.githubcopilot.com".to_string()
            ),
            (
                "Storage".to_string(),
                "Managed profile home created; token remains in Copilot's keychain/config store."
                    .to_string()
            ),
            ("Active".to_string(), "copilot-main".to_string()),
        ]
    );

    assert_eq!(
        copilot_profile_import_summary_fields(CopilotProfileImportSummary {
            profile_name: "copilot-main".to_string(),
            provider: "GitHub Copilot".to_string(),
            identity: "octo".to_string(),
            github_host: "https://github.com".to_string(),
            api_url: None,
            codex_home: None,
            active: false,
            updated_existing: true,
        })
        .first()
        .map(|(_, value)| value.as_str()),
        Some("Updated imported Copilot profile 'copilot-main'.")
    );
}
