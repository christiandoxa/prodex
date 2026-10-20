use super::*;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

#[test]
fn quota_runtime_profile_envelopes_keep_bracket_and_bare_event_contracts() {
    for (line, expected) in [
        ("[β] token_usage profile=alpha] next=value", Some("alpha]")),
        ("[] profile_commit profile=βeta", Some("βeta")),
        ("token_usage profile=alpha", Some("alpha")),
        ("prefix] token_usage profile=alpha", None),
        ("[unterminated token_usage profile=alpha", None),
        ("[x]token_usage profile=alpha", None),
        ("[x] selection_pick profile=alpha", None),
        (
            r#"{"event":"profile_commit","fields":{"profile":"json] profile"}}"#,
            Some("json] profile"),
        ),
    ] {
        assert_eq!(
            quota_runtime_profile_from_line(line).as_deref(),
            expected,
            "{line:?}"
        );
    }
}

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
fn quota_provider_filter_uses_mojo_casefold_identity() {
    let auth_only = |label: &str| QuotaReport {
        name: "test".to_string(),
        active: false,
        auth: AuthSummary {
            label: label.to_string(),
            quota_compatible: true,
        },
        workspace_id: None,
        workspace_name: None,
        result: Err("not fetched".to_string()),
        fetched_at: 0,
    };
    assert!(QuotaProviderFilter::OpenAi.matches_report(&auth_only("CHATGPT")));
    assert!(QuotaProviderFilter::Gemini.matches_report(&auth_only("GeMiNi")));
    assert!(QuotaProviderFilter::Anthropic.matches_report(&auth_only("ANTHROPIC")));
    assert!(QuotaProviderFilter::Copilot.matches_report(&auth_only("CoPiLoT")));
    assert!(QuotaProviderFilter::Kiro.matches_report(&auth_only("KIRO")));
    assert!(QuotaProviderFilter::DeepSeek.matches_report(&auth_only("DEEPSEEK-KEY")));
    assert!(QuotaProviderFilter::Local.matches_report(&auth_only("LOCAL")));
    assert!(!QuotaProviderFilter::OpenAi.matches_report(&auth_only(" chatgpt ")));

    let external = |provider: &str| QuotaReport {
        name: "external".to_string(),
        active: false,
        auth: AuthSummary {
            label: "other".to_string(),
            quota_compatible: true,
        },
        workspace_id: None,
        workspace_name: None,
        result: Ok(ProviderQuotaSnapshot::External(ExternalQuotaInfo {
            provider: provider.to_string(),
            account: None,
            plan: None,
            status: "ok".to_string(),
            main: "ok".to_string(),
            reset: None,
            available: Some(true),
            details: Vec::new(),
        })),
        fetched_at: 0,
    };
    assert!(QuotaProviderFilter::DeepSeek.matches_report(&external("dEePsEeK")));
    assert!(QuotaProviderFilter::Local.matches_report(&external("LOCAL OPENAI-COMPATIBLE")));
    assert!(!QuotaProviderFilter::Local.matches_report(&external(" Local OpenAI-compatible ")));
}

#[test]
fn compiled_quota_runtime_log_order_selects_newest_known_profile_regardless_of_input_order() {
    let older = test_runtime_log_path("a-older.log");
    let root = older.parent().unwrap().to_path_buf();
    let newer = root.join("z-newer.log");
    let missing = root.join("missing.log");
    for (path, profile_name, timestamp) in [(&older, "active", 10_u64), (&newer, "runtime", 20_u64)]
    {
        fs::write(
            path,
            format!(
                "{}\n",
                serde_json::json!({
                    "event": "profile_commit",
                    "fields": {"profile": profile_name},
                })
            ),
        )
        .unwrap();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(timestamp))
            .unwrap();
    }
    let state = AppState {
        active_profile: Some("active".into()),
        profiles: BTreeMap::from([
            ("active".into(), profile("/tmp/active")),
            ("runtime".into(), profile("/tmp/runtime")),
        ]),
        ..AppState::default()
    };
    for paths in [
        vec![newer.clone(), older.clone(), missing.clone()],
        vec![missing, older.clone(), newer.clone()],
    ] {
        assert_eq!(
            quota_current_runtime_profile_name_from_paths(&state, paths).as_deref(),
            Some("runtime"),
        );
    }
    fs::write(
        &newer,
        r#"{"event":"profile_commit","fields":{"profile":"deleted"}}"#,
    )
    .unwrap();
    assert_eq!(
        quota_current_runtime_profile_name_from_paths(&state, [newer, older]).as_deref(),
        Some("active"),
        "the latest unknown profile cannot erase the last recognized one",
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_quota_runtime_log_order_breaks_equal_timestamps_by_path_ascending() {
    let alpha = test_runtime_log_path("a-alpha.log");
    let root = alpha.parent().unwrap().to_path_buf();
    let zeta = root.join("z-雪.log");
    for (path, profile_name) in [(&alpha, "active"), (&zeta, "runtime")] {
        fs::write(
            path,
            format!(
                "{}\n",
                serde_json::json!({
                    "event": "token_usage",
                    "fields": {"profile": profile_name},
                })
            ),
        )
        .unwrap();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(UNIX_EPOCH + std::time::Duration::from_secs(90))
            .unwrap();
    }
    let state = AppState {
        profiles: BTreeMap::from([
            ("active".into(), profile("/tmp/active")),
            ("runtime".into(), profile("/tmp/runtime")),
        ]),
        ..AppState::default()
    };
    assert_eq!(
        quota_current_runtime_profile_name_from_paths(&state, [zeta, alpha]).as_deref(),
        Some("runtime"),
        "the lexically last equally-timed path supplies the latest event",
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_quota_state_profile_selection_prefers_latest_valid_and_unicode_name_ties() {
    let state = AppState {
        active_profile: Some("active".into()),
        profiles: BTreeMap::from([
            ("active".into(), profile("/tmp/active")),
            ("z".into(), profile("/tmp/z")),
            ("é".into(), profile("/tmp/accent")),
            ("雪".into(), profile("/tmp/snow")),
            ("ancient".into(), profile("/tmp/ancient")),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("active".into(), 10),
            ("z".into(), 20),
            ("é".into(), 20),
            ("雪".into(), 20),
            ("deleted".into(), i64::MAX),
            ("ancient".into(), i64::MIN),
        ]),
        ..AppState::default()
    };
    assert_eq!(
        quota_state_current_profile_name(&state).as_deref(),
        Some("雪")
    );
    let mut state = state;
    state.last_run_selected_at.insert("雪".into(), -5);
    assert_eq!(
        quota_state_current_profile_name(&state).as_deref(),
        Some("é")
    );
    state.last_run_selected_at.remove("é");
    assert_eq!(
        quota_state_current_profile_name(&state).as_deref(),
        Some("z")
    );
}

#[test]
fn compiled_quota_state_profile_selection_requires_existing_active_fallback() {
    let mut state = AppState {
        active_profile: Some("active".into()),
        profiles: BTreeMap::from([("active".into(), profile("/tmp/active"))]),
        last_run_selected_at: BTreeMap::from([("deleted".into(), i64::MAX)]),
        ..AppState::default()
    };
    assert_eq!(
        quota_state_current_profile_name(&state).as_deref(),
        Some("active")
    );
    state.profiles.clear();
    assert_eq!(quota_state_current_profile_name(&state), None);
    state.active_profile = None;
    state.last_run_selected_at.clear();
    assert_eq!(quota_state_current_profile_name(&state), None);
    state
        .profiles
        .insert("active".into(), profile("/tmp/active"));
    state.last_run_selected_at.insert("active".into(), i64::MIN);
    assert_eq!(
        quota_state_current_profile_name(&state).as_deref(),
        Some("active")
    );
}
