use super::*;

#[test]
fn parses_identity_from_auth_json_with_id_token_account_priority() {
    let identity = parse_identity_from_auth_json(&format!(
        r#"{{
                "tokens": {{
                    "id_token": "{}",
                    "access_token": "{}",
                    "account_id": "stored-account"
                }}
            }}"#,
        chatgpt_id_token("user@example.com", Some("id-token-account")),
        chatgpt_access_token_with_auth_object("access-token-account"),
    ))
    .unwrap();

    assert_eq!(identity.email.as_deref(), Some("user@example.com"));
    assert_eq!(identity.account_id.as_deref(), Some("id-token-account"));
}

#[test]
fn parses_access_token_account_before_stored_account() {
    let identity = parse_identity_from_auth_json(&format!(
        r#"{{
                "tokens": {{
                    "id_token": "{}",
                    "access_token": "{}",
                    "account_id": "stored-account"
                }}
            }}"#,
        chatgpt_id_token("user@example.com", None),
        chatgpt_access_token_with_auth_object("access-token-account"),
    ))
    .unwrap();

    assert_eq!(identity.account_id.as_deref(), Some("access-token-account"));
}

#[test]
fn parses_v2_personal_access_token_identity_from_stored_account() {
    let identity = parse_identity_from_auth_json(&format!(
        r#"{{
                "auth_mode": "personalAccessToken",
                "tokens": {{
                    "id_token": "{}",
                    "access_token": "at-v2-opaque-token",
                    "account_id": "stored-account"
                }}
            }}"#,
        chatgpt_id_token("user@example.com", None),
    ))
    .unwrap();

    assert_eq!(identity.email.as_deref(), Some("user@example.com"));
    assert_eq!(identity.account_id.as_deref(), Some("stored-account"));
}

#[test]
fn parses_access_token_flat_account_claims() {
    let flat_claim = jwt_with_payload(serde_json::json!({
        "https://api.openai.com/auth.chatgpt_account_id": "flat-account",
    }));
    let legacy_claim = jwt_with_payload(serde_json::json!({
        "chatgpt_account_id": "legacy-account",
    }));

    assert_eq!(
        parse_account_id_from_access_token(&flat_claim)
            .unwrap()
            .as_deref(),
        Some("flat-account")
    );
    assert_eq!(
        parse_account_id_from_access_token(&legacy_claim)
            .unwrap()
            .as_deref(),
        Some("legacy-account")
    );
}

#[test]
fn parses_email_from_profile_or_top_level_claim() {
    let top_level = jwt_with_payload(serde_json::json!({
        "email": " top@example.com ",
    }));
    let profile = jwt_with_payload(serde_json::json!({
        "https://api.openai.com/profile": {
            "email": " profile@example.com ",
        },
    }));

    assert_eq!(
        parse_email_from_id_token(&top_level).unwrap().as_deref(),
        Some("top@example.com")
    );
    assert_eq!(
        parse_email_from_id_token(&profile).unwrap().as_deref(),
        Some("profile@example.com")
    );
}

#[test]
fn rejects_invalid_jwt_shape() {
    let err = parse_email_from_id_token("not-a-jwt").unwrap_err();

    assert!(format!("{err:#}").contains("invalid JWT format"));
}

#[test]
fn normalizes_email_and_account_id() {
    assert_eq!(normalize_email(" User@Example.COM "), "user@example.com");
    assert_eq!(
        normalize_optional_email("  user@example.com  ").as_deref(),
        Some("user@example.com")
    );
    assert_eq!(normalize_optional_email("   "), None);
    assert_eq!(normalize_account_id(" acct-one "), "acct-one");
    assert_eq!(
        normalize_optional_account_id(" acct-two ").as_deref(),
        Some("acct-two")
    );
    assert_eq!(normalize_optional_account_id("   "), None);
}

#[test]
fn identity_match_requires_email_when_account_id_matches() {
    let discovered = vec![
        (
            "legacy".to_string(),
            ProfileIdentity {
                email: Some("User@Example.com".to_string()),
                account_id: None,
            },
        ),
        (
            "workspace".to_string(),
            ProfileIdentity {
                email: Some("other@example.com".to_string()),
                account_id: Some(" acct-one ".to_string()),
            },
        ),
    ];

    assert_eq!(
        find_matching_profile_identity(
            &discovered,
            &ProfileIdentity {
                email: Some("other@example.com".to_string()),
                account_id: Some("acct-one".to_string()),
            },
        )
        .as_deref(),
        Some("workspace")
    );
    assert_eq!(
        find_matching_profile_identity(
            &discovered,
            &ProfileIdentity {
                email: Some("different@example.com".to_string()),
                account_id: Some("acct-one".to_string()),
            },
        ),
        None
    );
    assert_eq!(
        find_matching_profile_identity(
            &discovered,
            &ProfileIdentity {
                email: Some("user@example.com".to_string()),
                account_id: Some("missing-account".to_string()),
            },
        )
        .as_deref(),
        Some("legacy")
    );
}

#[test]
fn identity_match_rejects_ambiguous_legacy_email() {
    let discovered = vec![
        (
            "first".to_string(),
            ProfileIdentity {
                email: Some("user@example.com".to_string()),
                account_id: None,
            },
        ),
        (
            "second".to_string(),
            ProfileIdentity {
                email: Some("USER@example.com".to_string()),
                account_id: None,
            },
        ),
    ];

    assert_eq!(
        find_matching_profile_identity(
            &discovered,
            &ProfileIdentity {
                email: Some("user@example.com".to_string()),
                account_id: None,
            },
        ),
        None
    );
}

#[test]
fn unique_profile_name_helpers_pick_first_available_candidate() {
    assert_eq!(
        unique_profile_name_for_email("User+Work@example.com", |candidate| {
            candidate == "user-work_example.com-3"
        }),
        "user-work_example.com-3"
    );
    assert_eq!(copilot_profile_name_base("User.Name"), "copilot-user.name");
    assert_eq!(
        unique_copilot_profile_name("User Name", |candidate| candidate == "copilot-user-name-2"),
        "copilot-user-name-2"
    );
    assert_eq!(
        unique_profile_name_from_base("", "copilot", |candidate| candidate == "copilot-2"),
        "copilot-2"
    );
}

#[test]
fn detects_email_derived_profile_name_for_other_email() {
    assert!(!profile_name_looks_email_derived_for_other_email(
        "main_example.com",
        "main@example.com"
    ));
    assert!(!profile_name_looks_email_derived_for_other_email(
        "main_example.com-2",
        "main@example.com"
    ));
    assert!(!profile_name_looks_email_derived_for_other_email(
        "primary",
        "main@example.com"
    ));
    assert!(profile_name_looks_email_derived_for_other_email(
        "customer_example.com",
        "team@example.com"
    ));
}

#[test]
fn profile_name_validation_matches_cli_rules() {
    validate_profile_name("main.profile_1").expect("valid profile name");

    assert_eq!(
        validate_profile_name("").unwrap_err().to_string(),
        "profile name cannot be empty"
    );
    assert_eq!(
        validate_profile_name("bad/name").unwrap_err().to_string(),
        "profile name cannot contain path separators"
    );
    assert_eq!(
        validate_profile_name("..").unwrap_err().to_string(),
        "profile name cannot be '.' or '..'"
    );
    assert_eq!(
        validate_profile_name("bad name").unwrap_err().to_string(),
        "profile name may only contain letters, numbers, '.', '_' or '-'"
    );
}

#[test]
fn add_profile_option_validation_rejects_conflicting_sources() {
    assert_eq!(
        validate_add_profile_options(true, false, true)
            .unwrap_err()
            .to_string(),
        "--codex-home cannot be combined with --copy-from or --copy-current"
    );
    assert_eq!(
        validate_add_profile_options(false, true, true)
            .unwrap_err()
            .to_string(),
        "use either --copy-from or --copy-current"
    );
    validate_add_profile_options(false, true, false).expect("copy-from alone should pass");
}

#[test]
fn add_profile_source_kind_and_activation_are_planned() {
    assert_eq!(
        resolve_add_profile_source_kind(true, false, false).unwrap(),
        AddProfileSourceKind::ExternalHome
    );
    assert_eq!(
        resolve_add_profile_source_kind(false, true, false).unwrap(),
        AddProfileSourceKind::CopyFrom
    );
    assert_eq!(
        resolve_add_profile_source_kind(false, false, true).unwrap(),
        AddProfileSourceKind::CopyCurrent
    );
    assert_eq!(
        resolve_add_profile_source_kind(false, false, false).unwrap(),
        AddProfileSourceKind::EmptyManaged
    );
    assert!(AddProfileSourceKind::EmptyManaged.managed());
    assert!(!AddProfileSourceKind::ExternalHome.managed());
    assert!(should_activate_profile(false, false));
    assert!(should_activate_profile(true, true));
    assert!(!should_activate_profile(true, false));
}

#[test]
fn remove_profile_target_planning_validates_bulk_delete() {
    let profiles = [("main", true), ("external", false)];

    assert_eq!(
        resolve_remove_profile_targets(profiles, false, Some("main"), false).unwrap(),
        vec!["main".to_string()]
    );
    assert_eq!(
        resolve_remove_profile_targets(profiles, false, Some("missing"), false)
            .unwrap_err()
            .to_string(),
        "profile 'missing' does not exist"
    );
    assert_eq!(
        resolve_remove_profile_targets(profiles, true, None, false).unwrap(),
        vec!["main".to_string(), "external".to_string()]
    );
    assert_eq!(
        resolve_remove_profile_targets(profiles, true, None, true)
            .unwrap_err()
            .to_string(),
        "--delete-home with --all refuses to delete external profiles: external"
    );
}

#[test]
fn profile_home_delete_and_removed_state_are_planned() {
    assert!(should_delete_profile_home(true, false, "/tmp/managed").unwrap());
    assert!(!should_delete_profile_home(false, false, "/tmp/external").unwrap());
    assert_eq!(
        should_delete_profile_home(false, true, "/tmp/external")
            .unwrap_err()
            .to_string(),
        "refusing to delete external path /tmp/external"
    );

    let plan = plan_removed_profile_state(["backup"], Some("main"), ["main"]);
    assert_eq!(plan.active_profile.as_deref(), Some("backup"));
    assert!(plan.removed_names.contains("main"));

    let plan = plan_removed_profile_state(["main", "backup"], Some("backup"), ["main"]);
    assert_eq!(plan.active_profile.as_deref(), Some("backup"));
}

fn chatgpt_id_token(email: &str, account_id: Option<&str>) -> String {
    let mut auth = serde_json::Map::new();
    if let Some(account_id) = account_id {
        auth.insert(
            "chatgpt_account_id".to_string(),
            serde_json::Value::String(account_id.to_string()),
        );
    }
    jwt_with_payload(serde_json::json!({
        "https://api.openai.com/profile": {
            "email": email
        },
        "https://api.openai.com/auth": auth,
    }))
}

fn chatgpt_access_token_with_auth_object(account_id: &str) -> String {
    jwt_with_payload(serde_json::json!({
        "https://api.openai.com/auth": {
            "chatgpt_account_id": account_id,
        },
    }))
}

fn jwt_with_payload(payload: serde_json::Value) -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string());
    format!("{header}.{payload}.sig")
}
