use super::*;

#[test]
fn copilot_config_parser_treats_empty_file_as_logged_out() {
    let config = parse_copilot_config_file(" \n\t")
        .expect("blank config should parse as empty Copilot state");

    assert!(
        select_copilot_logged_in_user(&config)
            .expect("empty Copilot state should resolve")
            .is_none()
    );
    assert!(config.logged_in_users.is_empty());
    assert!(config.copilot_tokens.is_empty());
}

#[test]
fn copilot_config_debug_redacts_and_zeroize_clears_tokens() {
    let mut config = parse_copilot_config_file(
        r#"{
            "lastLoggedInUser": {"host": "https://github.com", "login": "main"},
            "copilotTokens": {"https://github.com:main": "copilot-debug-secret"}
        }"#,
    )
    .expect("Copilot config should parse");

    let rendered = format!("{config:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("copilot-debug-secret"));

    config.zeroize();
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

    let user = select_copilot_logged_in_user(&config)
        .expect("Copilot JSONC candidate resolution")
        .expect("user should resolve");
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

    let user = select_copilot_logged_in_user(&config)
        .expect("Copilot candidate resolution")
        .expect("user should resolve");

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
fn copilot_import_candidates_are_mojo_deduplicated_in_preferred_order() {
    let config = CopilotConfigFile {
        last_logged_in_user: Some(CopilotConfigUser {
            host: "https://ghe.example".to_string(),
            login: "main".to_string(),
        }),
        logged_in_users: vec![
            CopilotConfigUser {
                host: "https://ghe.example".to_string(),
                login: "main".to_string(),
            },
            CopilotConfigUser {
                host: "https://github.com".to_string(),
                login: "fallback".to_string(),
            },
            CopilotConfigUser {
                host: "https://github.com".to_string(),
                login: "fallback".to_string(),
            },
        ],
        copilot_tokens: BTreeMap::new(),
    };

    assert_eq!(
        resolve_copilot_import_candidates(&config).expect("candidate resolution"),
        vec![
            CopilotConfigUser {
                host: "https://ghe.example".to_string(),
                login: "main".to_string(),
            },
            CopilotConfigUser {
                host: "https://github.com".to_string(),
                login: "fallback".to_string(),
            },
        ]
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
        default_copilot_models_api_url("HTTPS://GITHUB.COM/"),
        "https://api.githubcopilot.com"
    );
    assert_eq!(
        default_copilot_models_api_url("https://enterprise.ghe.com"),
        "https://copilot-api.enterprise.ghe.com"
    );
    assert_eq!(
        copilot_user_api_origin("https://api.example.test/path?x=1").unwrap(),
        "https://api.example.test"
    );
    assert_eq!(
        copilot_user_api_origin("https://example.test:8443/path").unwrap(),
        "https://example.test:8443"
    );
    assert_eq!(
        copilot_user_api_origin("http://[::1]:1234/path").unwrap(),
        "http://[::1]:1234"
    );
    assert_eq!(
        copilot_user_api_origin("https://").unwrap(),
        "https://api.https:"
    );
    assert!(copilot_user_api_origin("://host").is_err());
    assert!(copilot_user_api_origin("   ").is_err());
    assert_eq!(
        default_copilot_models_api_url("https://Enterprise.GHE.com"),
        "https://api.Enterprise.GHE.com"
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
