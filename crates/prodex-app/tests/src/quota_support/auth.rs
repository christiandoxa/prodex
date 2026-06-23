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
