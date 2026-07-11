use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn non_openai_model_provider_disables_quota_summary() {
    let root = temp_dir("non-openai-model-provider");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    fs::write(
        secret_store::auth_json_path(&root),
        r#"{"tokens":{"access_token":"test-token"}}"#,
    )
    .unwrap();

    let summary = read_auth_summary(&root);

    assert_eq!(summary.label, "model-provider:amazon-bedrock");
    assert!(!summary.quota_compatible);
}

#[test]
fn reset_credit_consume_response_defaults_missing_outcome_to_reset() {
    let response: RateLimitResetCreditConsumeResponse =
        serde_json::from_str(r#"{"rate_limit_reset_credits":{"available_count":0}}"#).unwrap();

    assert_eq!(response.outcome, RateLimitResetCreditConsumeOutcome::Reset);
}

#[test]
fn reset_credit_consume_response_preserves_explicit_outcome() {
    let response: RateLimitResetCreditConsumeResponse =
        serde_json::from_str(r#"{"outcome":"nothingToReset"}"#).unwrap();

    assert_eq!(
        response.outcome,
        RateLimitResetCreditConsumeOutcome::NothingToReset
    );
}

#[test]
fn accounts_response_extracts_workspace_names() {
    let list: ChatgptAccountsResponse = serde_json::from_str(
        r#"{"accounts":[{"id":"acct_personal","structure":"personal"},{"id":"acct_team","name":"Team Workspace","structure":"workspace"}]}"#,
    )
    .unwrap();
    let accounts = list.accounts();
    assert_eq!(accounts[0].display_name().as_deref(), Some("Personal"));
    assert_eq!(
        accounts[1].display_name().as_deref(),
        Some("Team Workspace")
    );

    let map: ChatgptAccountsResponse = serde_json::from_str(
        r#"{"account_ordering":["acct_team"],"accounts":{"acct_team":{"account":{"account_id":"acct_team","name":"Team Workspace","structure":"workspace"}}}}"#,
    )
    .unwrap();
    assert_eq!(
        map.accounts()[0].display_name().as_deref(),
        Some("Team Workspace")
    );

    let unordered_map: ChatgptAccountsResponse = serde_json::from_str(
        r#"{"accounts":{"acct_team":{"account":{"account_id":"acct_team","name":"Team Workspace","structure":"workspace"}}}}"#,
    )
    .unwrap();
    assert_eq!(
        unordered_map.accounts()[0].display_name().as_deref(),
        Some("Team Workspace")
    );
}

#[test]
fn accounts_check_url_matches_codex_backend_style() {
    assert_eq!(
        chatgpt_accounts_check_url("https://chatgpt.com/backend-api"),
        "https://chatgpt.com/backend-api/wham/accounts/check"
    );
    assert_eq!(
        chatgpt_accounts_check_url("http://127.0.0.1:8080"),
        "http://127.0.0.1:8080/api/codex/accounts/check"
    );
}

#[test]
fn codex_openai_auth_headers_match_upstream_metadata_shape() {
    let request =
        codex_openai_auth_headers(reqwest::blocking::Client::new().get("http://127.0.0.1/"))
            .build()
            .unwrap();
    let headers = request.headers();

    assert_eq!(
        headers
            .get("originator")
            .and_then(|value| value.to_str().ok()),
        Some("codex_cli_rs")
    );
    if let Some(user_agent) = headers
        .get(reqwest::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
    {
        assert!(user_agent.starts_with("codex_cli_rs/"));
        assert!(!user_agent.contains("prodex"));
    }
}

#[test]
fn codex_openai_auth_headers_include_configured_residency() {
    let root = temp_dir("auth-headers-residency");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "enforce_residency = 'us'\n").unwrap();

    let request = codex_openai_auth_headers_for_home(
        reqwest::blocking::Client::new().get("http://127.0.0.1/"),
        &root,
    )
    .build()
    .unwrap();

    assert_eq!(
        request
            .headers()
            .get("x-openai-internal-codex-residency")
            .and_then(|value| value.to_str().ok()),
        Some("us")
    );
}

#[test]
fn codex_openai_auth_user_agent_uses_codex_version_shape() {
    let user_agent = codex_openai_auth_user_agent_for_version(
        "codex_cli_rs",
        "0.142.5",
        "xterm-256color".to_string(),
    );

    assert!(user_agent.starts_with("codex_cli_rs/0.142.5 ("));
    assert!(user_agent.ends_with(") xterm-256color"));
    assert!(!user_agent.contains("prodex"));
}

#[test]
fn codex_cli_version_output_parses_codex_version() {
    assert_eq!(
        parse_codex_cli_version_output("codex-cli 0.142.5\n").as_deref(),
        Some("0.142.5")
    );
}

#[test]
fn codex_openai_auth_originator_honors_upstream_override() {
    let _env_lock = crate::test_support::TestEnvVarGuard::lock();
    let _originator = crate::test_support::TestEnvVarGuard::set(
        "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
        "codex-tui",
    );

    assert_eq!(codex_openai_auth_originator(), "codex-tui");
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "prodex-auth-summary-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    dir
}
