use super::*;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

fn profile(path: &str) -> ProfileEntry {
    ProfileEntry {
        codex_home: PathBuf::from(path),
        managed: true,
        email: None,
        provider: ProfileProvider::Openai,
    }
}

#[test]
fn quota_error_message_redacts_secret_like_material() {
    let err = anyhow::anyhow!(
        "failed: Authorization: Bearer fixture-token-123 url=https://example.test?api_key=sk-fixture-123"
    );

    let message = quota_error_message(&err);

    assert!(message.contains("Authorization: Bearer <redacted>"));
    assert!(message.contains("api_key=<redacted>"));
    assert!(!message.contains("fixture-token-123"));
    assert!(!message.contains("sk-fixture-123"));
}

fn test_runtime_log_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = env::temp_dir().join(format!("prodex-quota-test-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir.join(name)
}

#[test]
fn imported_kiro_profile_has_status_and_json_quota_snapshots() {
    let auth_path = test_runtime_log_path("kiro_auth.json");
    let codex_home = auth_path.parent().unwrap();
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(
            &secret_store::SecretLocation::file(&auth_path),
            serde_json::to_string(&serde_json::json!({
            "auth_key": "kirocli:fixture:token",
            "auth_kind": "social",
            "auth_json": "{\"accessToken\":\"fixture-token\"}",
            "email": "developer@example.com",
            "profile_name": "example-profile",
            "region": "us-east-1"
            }))
            .unwrap(),
        )
        .unwrap();
    fs::write(
        codex_home.join(KIRO_MODEL_CATALOG_FILE),
        br#"{"models":[{"id":"model-a"},{"id":"model-b"}]}"#,
    )
    .unwrap();
    let provider = ProfileProvider::Kiro {
        auth_key: "kirocli:fixture:token".to_string(),
        auth_kind: Some("social".to_string()),
        profile_arn: None,
        profile_name: Some("example-profile".to_string()),
        start_url: None,
        region: Some("us-east-1".to_string()),
    };

    let snapshot = fetch_profile_quota(&provider, codex_home, None).unwrap();
    let ProviderQuotaSnapshot::External(info) = snapshot else {
        panic!("expected external Kiro status snapshot");
    };
    assert_eq!(info.provider, "Kiro CLI");
    assert_eq!(info.main, "2 imported models");
    assert_eq!(info.account.as_deref(), Some("developer@example.com"));

    let json = fetch_profile_quota_json(&provider, codex_home, None).unwrap();
    assert_eq!(json["provider"], "Kiro CLI");
    assert_eq!(json["main"], "2 imported models");

    fs::remove_dir_all(codex_home).unwrap();
}

#[test]
fn quota_current_profile_prefers_latest_runtime_selection() {
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::from([
            ("active".to_string(), profile("/tmp/active")),
            ("runtime".to_string(), profile("/tmp/runtime")),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("active".to_string(), 10),
            ("runtime".to_string(), 20),
        ]),
        ..AppState::default()
    };

    assert_eq!(
        quota_current_profile_name(&state).as_deref(),
        Some("runtime")
    );
}

#[test]
fn quota_current_profile_falls_back_to_active_profile() {
    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::from([("active".to_string(), profile("/tmp/active"))]),
        last_run_selected_at: BTreeMap::from([("deleted".to_string(), 30)]),
        ..AppState::default()
    };

    assert_eq!(
        quota_current_profile_name(&state).as_deref(),
        Some("active")
    );
}

#[test]
fn reset_credit_consume_url_matches_backend_path_style() {
    assert_eq!(
        rate_limit_reset_credit_consume_url("https://chatgpt.com/backend-api"),
        "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume"
    );
    assert_eq!(
        rate_limit_reset_credit_consume_url("http://127.0.0.1:8080"),
        "http://127.0.0.1:8080/api/codex/rate-limit-reset-credits/consume"
    );
}

#[test]
fn quota_base_url_accepts_safe_url_and_normalizes_trailing_slashes() {
    assert_eq!(
        quota_base_url(Some("https://example.test/backend-api///")).unwrap(),
        "https://example.test/backend-api"
    );
}

#[test]
fn quota_base_url_rejects_credentials_query_and_fragment_without_echoing_values() {
    for value in [
        "https://quota-user-secret-sentinel@example.test/backend-api",
        "https://user:quota-password-secret-sentinel@example.test/backend-api",
        "https://example.test/backend-api?token=quota-query-secret-sentinel",
        "https://example.test/backend-api#quota-fragment-secret-sentinel",
    ] {
        let error = quota_base_url(Some(value)).unwrap_err().to_string();

        assert!(
            error.contains("no credentials, query, or fragment"),
            "{error}"
        );
        assert!(!error.contains("secret-sentinel"), "{error}");
    }
}

#[test]
fn quota_base_url_rejects_credential_bearing_environment_url_without_echoing_it() {
    let _base_url = crate::TestEnvVarGuard::set(
        "CODEX_CHATGPT_BASE_URL",
        "https://user:quota-env-secret-sentinel@example.test/backend-api",
    );

    let error = quota_base_url(None).unwrap_err().to_string();

    assert!(error.contains("CODEX_CHATGPT_BASE_URL"), "{error}");
    assert!(
        error.contains("no credentials, query, or fragment"),
        "{error}"
    );
    assert!(!error.contains("secret-sentinel"), "{error}");
}

#[test]
fn quota_current_profile_prefers_runtime_log_usage() {
    let log_path = test_runtime_log_path("prodex-runtime-test.log");
    let mut log = fs::File::create(&log_path).unwrap();
    writeln!(
            log,
            "[2026-06-22 16:00:00.000 +07:00] token_usage request=1 route=websocket transport=websocket profile=runtime source=responses_websocket"
        )
        .unwrap();

    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::from([
            ("active".to_string(), profile("/tmp/active")),
            ("runtime".to_string(), profile("/tmp/runtime")),
        ]),
        last_run_selected_at: BTreeMap::from([("active".to_string(), 30)]),
        ..AppState::default()
    };

    assert_eq!(
        quota_current_runtime_profile_name_from_paths(&state, vec![log_path]).as_deref(),
        Some("runtime")
    );
}

#[test]
fn quota_runtime_log_ignores_unknown_profiles() {
    let log_path = test_runtime_log_path("prodex-runtime-test.log");
    let mut log = fs::File::create(&log_path).unwrap();
    writeln!(
            log,
            "[2026-06-22 16:00:00.000 +07:00] token_usage request=1 route=websocket transport=websocket profile=deleted source=responses_websocket"
        )
        .unwrap();

    let state = AppState {
        active_profile: Some("active".to_string()),
        profiles: BTreeMap::from([("active".to_string(), profile("/tmp/active"))]),
        ..AppState::default()
    };

    assert_eq!(
        quota_current_runtime_profile_name_from_paths(&state, vec![log_path]),
        None
    );
}

#[test]
fn quota_runtime_auth_backoff_profiles_follow_runtime_log_clear() {
    let log_path = test_runtime_log_path("prodex-runtime-auth-test.log");
    let mut log = fs::File::create(&log_path).unwrap();
    writeln!(
            log,
            "[2026-06-22 16:00:00.000 +07:00] profile_auth_backoff profile=main route=responses status=401 score=100 seconds=300"
        )
        .unwrap();
    writeln!(
            log,
            "[2026-06-22 16:00:01.000 +07:00] profile_auth_backoff profile=second route=responses status=401 score=100 seconds=300"
        )
        .unwrap();
    writeln!(
            log,
            "[2026-06-22 16:00:02.000 +07:00] profile_auth_backoff_cleared profile=main reason=auth_changed"
        )
        .unwrap();

    let profiles = quota_runtime_auth_backoff_profiles_from_paths(vec![log_path]);

    assert!(!profiles.contains("main"));
    assert!(profiles.contains("second"));
}
