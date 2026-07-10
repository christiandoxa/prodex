use super::*;
#[path = "runtime_launch/arg0_cleanup.rs"]
mod arg0_cleanup;
#[path = "runtime_launch/openai_spark_context.rs"]
mod openai_spark_context;
#[path = "runtime_launch/preflight.rs"]
mod preflight;
#[path = "runtime_launch/profile_selection.rs"]
mod profile_selection;
#[path = "runtime_launch/provider_rewrite.rs"]
mod provider_rewrite;
#[path = "runtime_launch/proxy_state.rs"]
mod proxy_state;
#[path = "runtime_launch/routes.rs"]
mod routes;
#[path = "runtime_launch/run_command_strategy.rs"]
mod run_command_strategy;
#[path = "runtime_launch/super_runtime.rs"]
mod super_runtime;
#[test]
fn gateway_state_store_config_builds_postgres_backend_from_env() {
    let root = temp_dir("gateway-postgres-state-config");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _postgres = TestEnvVarGuard::set(
        "PRODEX_GATEWAY_POSTGRES_URL_TEST",
        "postgres://prodex:prodex@127.0.0.1:5432/prodex",
    );
    let paths = AppPaths::discover().unwrap();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.state.backend = Some("postgres".to_string());
    policy.state.postgres_url_env = Some("PRODEX_GATEWAY_POSTGRES_URL_TEST".to_string());
    let store = gateway_state_store_config(&paths, &policy).unwrap();
    match store {
        RuntimeGatewayStateStore::Postgres { url, state_path } => {
            assert_eq!(url, "postgres://prodex:prodex@127.0.0.1:5432/prodex");
            assert_eq!(
                state_path.display().to_string(),
                "postgres:PRODEX_GATEWAY_POSTGRES_URL_TEST"
            );
        }
        other => panic!("expected postgres gateway state backend, got {other:?}"),
    }
}
#[test]
fn gateway_state_store_config_builds_redis_backend_from_env() {
    let root = temp_dir("gateway-redis-state-config");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _redis = TestEnvVarGuard::set("PRODEX_GATEWAY_REDIS_URL_TEST", "redis://127.0.0.1:6379/0");
    let paths = AppPaths::discover().unwrap();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.state.backend = Some("redis".to_string());
    policy.state.redis_url_env = Some("PRODEX_GATEWAY_REDIS_URL_TEST".to_string());
    let store = gateway_state_store_config(&paths, &policy).unwrap();
    match store {
        RuntimeGatewayStateStore::Redis { url, state_path } => {
            assert_eq!(url, "redis://127.0.0.1:6379/0");
            assert_eq!(
                state_path.display().to_string(),
                "redis:PRODEX_GATEWAY_REDIS_URL_TEST"
            );
        }
        other => panic!("expected redis gateway state backend, got {other:?}"),
    }
}
#[test]
fn gateway_state_store_config_builds_sqlite_backend_relative_to_root() {
    let root = temp_dir("gateway-sqlite-state-config");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.state.backend = Some("sqlite".to_string());
    policy.state.sqlite_path = Some("var/gateway.sqlite3".to_string());
    let store = gateway_state_store_config(&paths, &policy).unwrap();
    match store {
        RuntimeGatewayStateStore::Sqlite { path } => {
            assert_eq!(path, paths.root.join("var/gateway.sqlite3"));
        }
        other => panic!("expected sqlite gateway state backend, got {other:?}"),
    }
}
#[test]
fn gateway_state_store_config_rejects_unknown_backend() {
    let root = temp_dir("gateway-invalid-state-config");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.state.backend = Some("mystery".to_string());
    let err = gateway_state_store_config(&paths, &policy).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("gateway.state.backend"));
    assert!(message.contains("mystery"));
}
#[test]
fn gateway_sso_config_builds_trusted_proxy_settings_from_env() {
    let _sso = TestEnvVarGuard::set("PRODEX_GATEWAY_SSO_TOKEN_TEST", "sso-shared-secret");
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.sso.proxy_token_env = Some("PRODEX_GATEWAY_SSO_TOKEN_TEST".to_string());
    policy.sso.user_header = Some("x-auth-request-email".to_string());
    policy.sso.default_role = Some("viewer".to_string());
    let config = gateway_sso_config(&policy).unwrap();
    assert!(config.proxy_token_hash.is_some());
    assert_eq!(config.token_header, "x-prodex-sso-token");
    assert_eq!(config.user_header, "x-auth-request-email");
    assert_eq!(config.default_role, RuntimeGatewayAdminRole::Viewer);
}
#[test]
fn gateway_sso_config_builds_oidc_settings() {
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.sso.oidc_issuer = Some("https://idp.example".to_string());
    policy.sso.oidc_audience = Some("prodex-gateway".to_string());
    policy.sso.oidc_jwks_url = Some("https://idp.example/.well-known/jwks.json".to_string());
    policy.sso.oidc_user_claim = Some("preferred_username".to_string());
    policy.sso.oidc_role_claim = Some("roles".to_string());
    policy.sso.oidc_key_prefixes_claim = Some("teams".to_string());
    policy.sso.default_role = Some("viewer".to_string());
    let config = gateway_sso_config(&policy).unwrap();
    let oidc = config.oidc.expect("OIDC config should be present");
    assert_eq!(oidc.issuer, "https://idp.example");
    assert_eq!(oidc.audience, "prodex-gateway");
    assert_eq!(
        oidc.jwks_url.as_deref(),
        Some("https://idp.example/.well-known/jwks.json")
    );
    assert_eq!(oidc.user_claim, "preferred_username");
    assert_eq!(oidc.role_claim, "roles");
    assert_eq!(oidc.key_prefixes_claim, "teams");
    assert_eq!(config.default_role, RuntimeGatewayAdminRole::Viewer);
}
#[test]
fn gateway_sso_config_rejects_oidc_issuer_without_audience() {
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.sso.oidc_issuer = Some("https://idp.example".to_string());
    let err = gateway_sso_config(&policy).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("gateway.sso.oidc_audience"));
}

#[test]
fn gateway_observability_config_adds_runtime_log_and_resolves_jsonl_path() {
    let root = temp_dir("gateway-observability-jsonl");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.observability.sinks = vec!["http".to_string()];
    policy.observability.jsonl_path = Some("logs/gateway.jsonl".to_string());
    let config = gateway_observability_config(&paths, &policy).unwrap();
    assert_eq!(
        config.sinks,
        vec![
            "http".to_string(),
            "runtime-log".to_string(),
            "jsonl".to_string()
        ]
    );
    assert_eq!(
        config.jsonl_path.unwrap(),
        paths.root.join("logs/gateway.jsonl")
    );
    assert_eq!(config.http_schema, "generic");
}

#[test]
fn gateway_observability_config_reads_http_bearer_token_env() {
    let root = temp_dir("gateway-observability-http");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _token = TestEnvVarGuard::set("PRODEX_GATEWAY_OBS_TOKEN_TEST", " obs-token ");
    let paths = AppPaths::discover().unwrap();
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.observability.http_endpoint = Some("https://otel.example/v1/traces".to_string());
    policy.observability.http_schema = Some("OTLP".to_string());
    policy.observability.http_bearer_token_env = Some("PRODEX_GATEWAY_OBS_TOKEN_TEST".to_string());
    let config = gateway_observability_config(&paths, &policy).unwrap();
    assert!(config.sinks.contains(&"runtime-log".to_string()));
    assert!(config.sinks.contains(&"http".to_string()));
    assert_eq!(
        config.http_endpoint.as_deref(),
        Some("https://otel.example/v1/traces")
    );
    assert_eq!(config.http_schema, "otlp");
    assert_eq!(config.http_bearer_token.as_deref(), Some("obs-token"));
}

#[test]
fn resolve_gateway_guardrail_config_normalizes_webhook_phases_and_token_env() {
    let _token = TestEnvVarGuard::set("PRODEX_GATEWAY_GUARDRAIL_TOKEN_TEST", " guard-token ");
    let args = GatewayArgs {
        command: None,
        listen: None,
        provider: None,
        base_url: None,
        api_key: None,
        auth_token: None,
        smart_context: false,
        presidio: false,
        no_presidio: false,
    };
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.guardrails.webhook_url = Some("https://guard.example/hook".to_string());
    policy.guardrails.webhook_phases = vec![
        " request ".to_string(),
        "Response".to_string(),
        "custom".to_string(),
    ];
    policy.guardrails.webhook_bearer_token_env =
        Some("PRODEX_GATEWAY_GUARDRAIL_TOKEN_TEST".to_string());
    policy.guardrails.webhook_fail_closed = Some(true);
    let config = resolve_gateway_guardrail_config(&args, &policy);
    assert_eq!(
        config.webhook.url.as_deref(),
        Some("https://guard.example/hook")
    );
    assert_eq!(
        config.webhook.phases,
        vec!["pre".to_string(), "post".to_string(), "custom".to_string()]
    );
    assert_eq!(config.webhook.bearer_token.as_deref(), Some("guard-token"));
    assert!(config.webhook.fail_closed);
}

#[test]
fn resolve_gateway_guardrail_config_presidio_cli_overrides_policy() {
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy.guardrails.presidio_redaction = Some(false);

    let enabled = resolve_gateway_guardrail_config(
        &GatewayArgs {
            command: None,
            listen: None,
            provider: None,
            base_url: None,
            api_key: None,
            auth_token: None,
            smart_context: false,
            presidio: true,
            no_presidio: false,
        },
        &policy,
    );
    assert!(enabled.presidio_redaction_enabled);

    policy.guardrails.presidio_redaction = Some(true);
    let disabled = resolve_gateway_guardrail_config(
        &GatewayArgs {
            command: None,
            listen: None,
            provider: None,
            base_url: None,
            api_key: None,
            auth_token: None,
            smart_context: false,
            presidio: false,
            no_presidio: true,
        },
        &policy,
    );
    assert!(!disabled.presidio_redaction_enabled);
}

#[test]
fn gateway_route_aliases_config_applies_strategy_and_model_cleanup() {
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    policy
        .route_aliases
        .push(prodex_runtime_policy::RuntimePolicyGatewayRouteAlias {
            alias: " fast ".to_string(),
            models: vec![
                " gpt-5 ".to_string(),
                "".to_string(),
                "gpt-5-mini".to_string(),
            ],
            strategy: Some("fallback".to_string()),
            model_metrics: Vec::new(),
        });
    let aliases = gateway_route_aliases_config(&policy, None);
    assert_eq!(aliases.len(), 1);
    assert_eq!(aliases[0].alias, "fast");
    assert_eq!(
        aliases[0].models,
        vec!["gpt-5".to_string(), "gpt-5-mini".to_string()]
    );
    assert_eq!(
        aliases[0].strategy,
        runtime_proxy_crate::RuntimeGatewayRouteStrategy::Fallback
    );
}

#[test]
fn gateway_route_alias_model_metrics_lets_policy_override_inferred_costs() {
    let metrics = gateway_route_alias_model_metrics(
        Some(prodex_provider_core::ProviderId::OpenAi),
        &[String::from("gpt-5")],
        &[
            prodex_runtime_policy::RuntimePolicyGatewayRouteModelMetrics {
                model: "gpt-5".to_string(),
                input_cost_per_million_microusd: Some(123),
                output_cost_per_million_microusd: Some(456),
                latency_ms: Some(789),
                rpm_limit: Some(12),
                tpm_limit: Some(34),
            },
        ],
    );
    let metric = metrics.get("gpt-5").expect("metric should exist");
    assert_eq!(metric.input_cost_per_million_microusd, Some(123));
    assert_eq!(metric.output_cost_per_million_microusd, Some(456));
    assert_eq!(metric.latency_ms, Some(789));
    assert_eq!(metric.rpm_limit, Some(12));
    assert_eq!(metric.tpm_limit, Some(34));
}

#[test]
fn gateway_upstream_base_url_adds_v1_for_openai_root_url() {
    let args = GatewayArgs {
        command: None,
        listen: None,
        provider: None,
        base_url: Some("https://example.test/".to_string()),
        api_key: None,
        auth_token: None,
        smart_context: false,
        presidio: false,
        no_presidio: false,
    };
    let policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    let url = gateway_upstream_base_url(&args, &policy, None).unwrap();
    assert_eq!(url, "https://example.test/v1");
}

#[test]
fn gateway_openai_api_keys_prefers_multi_key_env() {
    let _single = TestEnvVarGuard::set("OPENAI_API_KEY", "single-key");
    let _multi = TestEnvVarGuard::set("OPENAI_API_KEYS", " first , second ,, ");
    let keys = gateway_openai_api_keys(None);
    assert_eq!(keys, vec!["first".to_string(), "second".to_string()]);
}

#[test]
fn resolve_gateway_auth_config_uses_cli_token_as_default_admin() {
    let args = GatewayArgs {
        command: None,
        listen: None,
        provider: None,
        base_url: None,
        api_key: None,
        auth_token: Some("cli-gateway-token".to_string()),
        smart_context: false,
        presidio: false,
        no_presidio: false,
    };
    let policy = prodex_runtime_policy::RuntimePolicyGatewaySettings::default();
    let config = resolve_gateway_auth_config(&args, &policy).unwrap();
    assert!(config.auth_required);
    assert!(config.auth_token_hash.is_some());
    assert_eq!(config.admin_tokens.len(), 1);
    assert_eq!(config.admin_tokens[0].name, "default-admin");
    assert_eq!(config.virtual_keys.len(), 0);
}

#[test]
fn resolve_gateway_auth_config_requires_non_empty_virtual_key_env_when_policy_demands_auth() {
    let _token = TestEnvVarGuard::set("PRODEX_GATEWAY_VKEY_EMPTY_TEST", "   ");
    let args = GatewayArgs {
        command: None,
        listen: None,
        provider: None,
        base_url: None,
        api_key: None,
        auth_token: None,
        smart_context: false,
        presidio: false,
        no_presidio: false,
    };
    let mut policy = prodex_runtime_policy::RuntimePolicyGatewaySettings {
        require_auth: Some(true),
        ..Default::default()
    };
    policy
        .virtual_keys
        .push(prodex_runtime_policy::RuntimePolicyGatewayVirtualKey {
            name: "tenant-key".to_string(),
            token_env: "PRODEX_GATEWAY_VKEY_EMPTY_TEST".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: Vec::new(),
            budget_usd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
        });
    let err = resolve_gateway_auth_config(&args, &policy).unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("tenant-key"));
    assert!(message.contains("cannot be empty"));
}
#[test]
fn prepare_runtime_launch_skips_proxy_for_non_openai_model_provider() {
    let root = temp_dir("skip-proxy-non-openai");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let bedrock_home = root.join("bedrock-home");
    let openai_home = root.join("openai-home");
    fs::create_dir_all(&bedrock_home).unwrap();
    fs::create_dir_all(&openai_home).unwrap();
    fs::write(
        bedrock_home.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    fs::write(
        secret_store::auth_json_path(&openai_home),
        r#"{"tokens":{"access_token":"chatgpt-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("bedrock".to_string()),
            profiles: BTreeMap::from([
                (
                    "bedrock".to_string(),
                    ProfileEntry {
                        codex_home: bedrock_home.clone(),
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "openai".to_string(),
                    ProfileEntry {
                        codex_home: openai_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            ..AppState::default()
        },
    );
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("bedrock"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, bedrock_home);
    assert!(prepared.runtime_proxy.is_none());
}
#[test]
fn prepare_runtime_launch_rejects_claude_for_non_openai_model_provider() {
    let root = temp_dir("reject-claude-non-openai");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let bedrock_home = root.join("bedrock-home");
    fs::create_dir_all(&bedrock_home).unwrap();
    fs::write(
        bedrock_home.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("bedrock".to_string()),
            profiles: BTreeMap::from([(
                "bedrock".to_string(),
                ProfileEntry {
                    codex_home: bedrock_home,
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );
    let err = match prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("bedrock"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: true,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    }) {
        Ok(_) => panic!("expected Claude launch to reject non-OpenAI model providers"),
        Err(err) => err,
    };
    let message = format!("{err:#}");
    assert!(message.contains("amazon-bedrock"));
    assert!(message.contains("prodex claude"));
}
#[test]
fn prepare_runtime_launch_dry_run_uses_proxy_preview_without_recording_selection() {
    let root = temp_dir("dry-run-preview-no-selection-save");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    let second_home = root.join("second-home");
    fs::create_dir_all(&main_home).unwrap();
    fs::create_dir_all(&second_home).unwrap();
    fs::write(
        secret_store::auth_json_path(&main_home),
        r#"{"tokens":{"access_token":"main-token"}}"#,
    )
    .unwrap();
    fs::write(
        secret_store::auth_json_path(&second_home),
        r#"{"tokens":{"access_token":"second-token"}}"#,
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([
                (
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home.clone(),
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
                (
                    "second".to_string(),
                    ProfileEntry {
                        codex_home: second_home,
                        managed: false,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                ),
            ]),
            ..AppState::default()
        },
    );
    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, main_home);
    assert_eq!(
        prepared
            .runtime_proxy
            .as_ref()
            .expect("runtime proxy preview should exist")
            .listen_addr
            .port(),
        0
    );
    let paths = AppPaths::discover().unwrap();
    let state = AppState::load(&paths).unwrap();
    assert!(
        state.last_run_selected_at.is_empty(),
        "dry-run must not record launch selection"
    );
}
#[test]
fn prepare_runtime_launch_allows_profileless_local_home_when_no_profiles_exist() {
    let root = temp_dir("profileless-local-home");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_root = root.join("shared-codex");
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some("prodex-local"),
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, paths.shared_codex_root);
    assert!(prepared.codex_home.is_dir());
    assert!(!prepared.managed);
    assert!(prepared.runtime_proxy.is_none());
    assert!(
        !paths.state_file.exists(),
        "profileless local launch should not persist synthetic profile selection"
    );
}
#[test]
fn prepare_runtime_launch_profile_v2_config_enables_profileless_local_rewrite_proxy() {
    let root = temp_dir("profile-v2-profileless-local-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_root = root.join("shared-codex");
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    fs::create_dir_all(&paths.shared_codex_root).unwrap();
    fs::write(
        paths.shared_codex_root.join("local.config.toml"),
        "model_provider = 'prodex-local'\n[model_providers.prodex-local]\nbase_url = 'http://127.0.0.1:8131/v1'\n",
    )
    .unwrap();
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(65_536),
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: Some("local"),
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, paths.shared_codex_root);
    assert!(!prepared.managed);
    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("profile-v2 prodex-local should use local rewrite proxy");
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_LOCAL_PROVIDER_ID)
    );
    assert_eq!(
        runtime_proxy.openai_mount_path,
        RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH
    );
}
#[test]
fn prepare_runtime_launch_enables_local_rewrite_proxy_for_prodex_local_smart_context() {
    let root = temp_dir("profileless-local-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_root = root.join("shared-codex");
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_root.to_str().unwrap());
    let paths = AppPaths::discover().unwrap();
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: Some("http://127.0.0.1:8131/v1"),
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(65_536),
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_LOCAL_PROVIDER_ID),
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, paths.shared_codex_root);
    assert!(prepared.codex_home.is_dir());
    assert!(!prepared.managed);
    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("prodex-local Smart Context should use local rewrite proxy");
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_LOCAL_PROVIDER_ID)
    );
    assert_eq!(
        runtime_proxy.openai_mount_path,
        RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH
    );
    assert!(
        !paths.state_file.exists(),
        "profileless local proxy launch should not persist synthetic profile selection"
    );
}
#[test]
fn prepare_runtime_launch_profileless_local_flag_preserves_existing_profiles() {
    let root = temp_dir("profileless-local-preserve-profile");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );
    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some("prodex-local"),
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, main_home);
    assert!(!prepared.managed);
    assert!(prepared.runtime_proxy.is_none());
}
#[test]
fn prepare_runtime_launch_uses_profile_v2_model_provider_overlay() {
    let root = temp_dir("profile-v2-provider-overlay");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    fs::write(main_home.join("config.toml"), "model_provider = 'openai'\n").unwrap();
    fs::write(
        main_home.join("bedrock.config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("main"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: Some("bedrock"),
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, main_home);
    assert!(prepared.runtime_proxy.is_none());
}
#[test]
fn prepare_runtime_launch_explicit_profile_keeps_profile_home_with_local_override() {
    let root = temp_dir("explicit-profile-local-override");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let main_home = root.join("main-home");
    fs::create_dir_all(&main_home).unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("main".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );
    let prepared = prepare_runtime_launch(RuntimeLaunchRequest {
        profile: Some("main"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_LOCAL_PROVIDER_ID),
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, main_home);
    assert!(!prepared.managed);
    assert!(prepared.runtime_proxy.is_none());
}
#[test]
fn prepare_runtime_launch_dry_run_skips_proxy_for_non_openai_model_provider() {
    let root = temp_dir("dry-run-skip-proxy-non-openai");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let bedrock_home = root.join("bedrock-home");
    fs::create_dir_all(&bedrock_home).unwrap();
    fs::write(
        bedrock_home.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    write_state(
        &root,
        AppState {
            active_profile: Some("bedrock".to_string()),
            profiles: BTreeMap::from([(
                "bedrock".to_string(),
                ProfileEntry {
                    codex_home: bedrock_home.clone(),
                    managed: false,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        },
    );
    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: Some("bedrock"),
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: false,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: None,
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    assert_eq!(prepared.codex_home, bedrock_home);
    assert!(prepared.runtime_proxy.is_none());
    let paths = AppPaths::discover().unwrap();
    let state = AppState::load(&paths).unwrap();
    assert!(
        state.last_run_selected_at.is_empty(),
        "dry-run must not record launch selection"
    );
}
#[test]
fn prepare_runtime_launch_dry_run_previews_local_rewrite_proxy_for_prodex_local_smart_context() {
    let root = temp_dir("dry-run-local-smart-context-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let prepared = prepare_runtime_launch_dry_run(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: Some("http://127.0.0.1:8131/v1"),
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: true,
        presidio_redaction_enabled: false,
        model_context_window_tokens: Some(65_536),
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: false,
        model_provider_override: Some(SUPER_LOCAL_PROVIDER_ID),
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    })
    .unwrap();
    let runtime_proxy = prepared
        .runtime_proxy
        .as_ref()
        .expect("dry-run should preview local rewrite proxy");
    assert_eq!(runtime_proxy.listen_addr.port(), 0);
    assert_eq!(
        runtime_proxy.local_model_provider_id.as_deref(),
        Some(SUPER_LOCAL_PROVIDER_ID)
    );
    assert_eq!(
        runtime_proxy.openai_mount_path,
        RUNTIME_LOCAL_REWRITE_PROXY_MOUNT_PATH
    );
    let paths = AppPaths::discover().unwrap();
    assert!(
        !paths.state_file.exists(),
        "dry-run local proxy preview must not persist synthetic profile selection"
    );
}
#[test]
fn prepare_runtime_launch_rejects_force_proxy_for_profileless_local_home() {
    let root = temp_dir("profileless-local-force-proxy");
    let _env = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let shared_root = root.join("shared-codex");
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_root.to_str().unwrap());
    let err = match prepare_runtime_launch(RuntimeLaunchRequest {
        profile: None,
        allow_auto_rotate: true,
        auto_redeem: false,
        skip_quota_check: true,
        base_url: None,
        upstream_no_proxy: false,
        include_code_review: false,
        smart_context_enabled: false,
        presidio_redaction_enabled: false,
        model_context_window_tokens: None,
        gemini_thinking_budget_tokens: None,
        force_runtime_proxy: true,
        model_provider_override: Some(SUPER_LOCAL_PROVIDER_ID),
        profile_v2_name: None,
        external_provider: None,
        external_provider_api_key: None,
    }) {
        Ok(_) => panic!("expected forced proxy launch to reject profileless local provider"),
        Err(err) => err,
    };
    let message = format!("{err:#}");
    assert!(message.contains(SUPER_LOCAL_PROVIDER_ID));
    assert!(message.contains("prodex claude"));
}

fn write_state(root: &Path, state: AppState) {
    fs::create_dir_all(root).unwrap();
    let paths = AppPaths::discover().unwrap();
    state.save(&paths).unwrap();
}
fn test_run_args(codex_args: Vec<OsString>) -> RunArgs {
    RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args,
    }
}
fn temp_dir(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "prodex-runtime-launch-{name}-{}-{}",
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

#[test]
fn post_exit_maintenance_stabilizes_history_image_attachment_paths() {
    let root = temp_dir("post-exit-history-attachment");
    let _home = TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _shared_override = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", "shared-codex");
    let paths = AppPaths::discover().expect("paths should resolve");
    let sessions_dir = paths.shared_codex_root.join("sessions/2026/06/24");
    let session_file = sessions_dir.join("rollout.jsonl");
    let image_source = root.join("codex-clipboard-history.png");
    fs::create_dir_all(&root).expect("root dir should exist");
    fs::create_dir_all(&sessions_dir).expect("sessions dir should exist");
    fs::write(&image_source, b"png bytes").expect("source image should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"timestamp":"2026-06-24T01:02:03Z","type":"event","payload":{{"content":[{{"type":"input_text","text":"pasted session text plus <image path=\"{}\">"}}]}}}}"#,
            image_source.display()
        ),
    )
    .expect("session should write");

    maintain_shared_codex_sessions_after_child_exit();

    let copied = paths
        .shared_codex_root
        .join("image_attachments/codex-clipboard-history.png");
    assert_eq!(
        fs::read(&copied).expect("stable image should exist"),
        b"png bytes"
    );
    let rewritten = fs::read_to_string(&session_file).expect("session should read");
    assert!(
        rewritten.contains("pasted session text plus"),
        "post-exit maintenance should preserve pasted session text: {rewritten}"
    );
    assert!(
        rewritten.contains(&copied.display().to_string()),
        "session should reference stable attachment path after post-exit maintenance: {rewritten}"
    );
    assert!(
        !rewritten.contains(&image_source.display().to_string()),
        "session should no longer reference transient clipboard image path: {rewritten}"
    );
}
