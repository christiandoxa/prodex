use super::*;

#[test]
fn runtime_proxy_lane_classifies_anthropic_messages_as_responses() {
    assert_eq!(
        runtime_proxy_request_lane("/v1/messages?beta=true", false),
        RuntimeRouteKind::Responses
    );
}

#[test]
fn runtime_proxy_long_lived_classifies_anthropic_messages_as_interactive_streams() {
    assert!(runtime_proxy_request_is_long_lived(
        "/v1/messages?beta=true",
        false
    ));
}

#[test]
fn runtime_proxy_interactive_wait_budget_extends_anthropic_messages() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(21, 42);
    let base_budget = ci_timing_upper_bound_ms(21, 42);
    assert_eq!(
        runtime_proxy_admission_wait_budget("/backend-api/codex/responses", false),
        base_budget
    );
    assert_eq!(
        runtime_proxy_admission_wait_budget("/v1/messages?beta=true", false),
        base_budget * RUNTIME_PROXY_INTERACTIVE_WAIT_MULTIPLIER as u32
    );
}

#[test]
fn runtime_proxy_claude_launch_env_uses_foundry_compat_with_profile_config_dir() {
    let temp_dir = TestDir::isolated();
    let _model_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_MODEL");
    let config_dir = temp_dir.path.join("claude-config");
    let codex_home = temp_dir.path.join("codex-home");
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43123"
            .parse()
            .expect("listen address should parse"),
        &config_dir,
        &codex_home,
    )
    .unwrap();
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "CLAUDE_CONFIG_DIR")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(config_dir.to_string_lossy().into_owned())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_BASE_URL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("http://127.0.0.1:43123".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_AUTH_TOKEN")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(PRODEX_CLAUDE_PROXY_API_KEY.to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "CLAUDE_CODE_USE_FOUNDRY")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("1".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_FOUNDRY_BASE_URL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("http://127.0.0.1:43123".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_FOUNDRY_API_KEY")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(PRODEX_CLAUDE_PROXY_API_KEY.to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("opus".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_DEFAULT_OPUS_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5.4".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("effort,max_effort,thinking,adaptive_thinking,interleaved_thinking".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_DEFAULT_SONNET_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5.3-codex".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_DEFAULT_HAIKU_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5.4-mini".to_string())
    );
    assert!(
        env.iter()
            .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION")
    );
    assert!(env.iter().all(|(key, _)| *key != "ANTHROPIC_API_KEY"));
    assert_eq!(
        runtime_proxy_claude_removed_env(),
        [
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
            "CLAUDE_CODE_USE_FOUNDRY",
            "CLAUDE_CODE_USE_ANTHROPIC_AWS",
            "ANTHROPIC_BEDROCK_BASE_URL",
            "ANTHROPIC_VERTEX_BASE_URL",
            "ANTHROPIC_FOUNDRY_BASE_URL",
            "ANTHROPIC_AWS_BASE_URL",
            "ANTHROPIC_FOUNDRY_RESOURCE",
            "ANTHROPIC_VERTEX_PROJECT_ID",
            "ANTHROPIC_AWS_WORKSPACE_ID",
            "CLOUD_ML_REGION",
            "ANTHROPIC_FOUNDRY_API_KEY",
            "ANTHROPIC_AWS_API_KEY",
            "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
            "CLAUDE_CODE_SKIP_VERTEX_AUTH",
            "CLAUDE_CODE_SKIP_FOUNDRY_AUTH",
            "CLAUDE_CODE_SKIP_ANTHROPIC_AWS_AUTH",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_OPUS_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_SONNET_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_DESCRIPTION",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL_SUPPORTED_CAPABILITIES",
            "ANTHROPIC_CUSTOM_MODEL_OPTION",
            "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME",
            "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION"
        ]
    );
}

#[test]
fn runtime_proxy_claude_launch_env_honors_model_override() {
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "gpt-5.4-mini");
    let temp_dir = TestDir::isolated();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
    )
    .unwrap();
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("haiku".to_string())
    );
    assert!(
        env.iter()
            .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION")
    );
}

#[test]
fn runtime_proxy_claude_launch_env_keeps_custom_picker_entry_for_unknown_override() {
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "my-gateway-model");
    let temp_dir = TestDir::isolated();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
    )
    .unwrap();
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("my-gateway-model".to_string())
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_CUSTOM_MODEL_OPTION")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("my-gateway-model".to_string())
    );
}

#[test]
fn runtime_proxy_claude_launch_env_uses_codex_config_model_by_default() {
    let _model_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_MODEL");
    let temp_dir = TestDir::isolated();
    let codex_home = temp_dir.path.join("codex-home");
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    fs::write(codex_home.join("config.toml"), "model = \"gpt-5.4\"\n")
        .expect("config should write");
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &codex_home,
    )
    .unwrap();
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("opus".to_string())
    );
    assert!(
        env.iter()
            .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION")
    );
}

#[test]
fn runtime_proxy_claude_launch_env_maps_alias_backed_override_to_builtin_picker_value() {
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "gpt-5.4");
    let temp_dir = TestDir::isolated();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
    )
    .unwrap();
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("opus".to_string())
    );
}

#[test]
fn runtime_proxy_claude_target_model_maps_builtin_aliases_to_pinned_gpt_models() {
    let _env_guard = TestEnvVarGuard::lock();
    assert_eq!(
        runtime_proxy_claude_target_model("opus"),
        "gpt-5.4".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("sonnet"),
        "gpt-5.3-codex".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("claude-sonnet-4-6"),
        "gpt-5.3-codex".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("haiku"),
        "gpt-5.4-mini".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("claude-opus-4-5"),
        "gpt-5.2".to_string()
    );
    assert_eq!(
        runtime_proxy_claude_target_model("claude-haiku-3-5"),
        "gpt-5.4-mini".to_string()
    );
}

#[test]
fn runtime_proxy_claude_model_mapping_matrix_stays_in_sync() {
    let _env_guard = TestEnvVarGuard::lock();
    for descriptor in runtime_proxy_responses_model_descriptors() {
        assert_eq!(
            runtime_proxy_responses_model_descriptor(descriptor.id).map(|entry| entry.id),
            Some(descriptor.id)
        );
        assert_eq!(
            runtime_proxy_responses_model_supports_xhigh(descriptor.id),
            descriptor.supports_xhigh
        );
        assert_eq!(
            runtime_proxy_responses_model_supported_effort_levels(descriptor.id),
            if descriptor.supports_xhigh {
                &["low", "medium", "high", "max"][..]
            } else {
                &["low", "medium", "high"][..]
            }
        );
        assert_eq!(
            runtime_proxy_claude_target_model(descriptor.id),
            descriptor.id
        );
        assert!(runtime_proxy_claude_managed_model_option_value(
            descriptor.id
        ));

        let expected_picker_value = descriptor
            .claude_alias
            .map(runtime_proxy_claude_alias_picker_value)
            .unwrap_or(descriptor.id);
        assert_eq!(
            runtime_proxy_claude_picker_model(descriptor.id),
            expected_picker_value
        );

        if let Some(alias) = descriptor.claude_alias {
            let alias_picker_value = runtime_proxy_claude_alias_picker_value(alias);
            assert!(runtime_proxy_claude_managed_model_option_value(
                alias_picker_value
            ));
            assert_eq!(
                runtime_proxy_claude_picker_model_descriptor(alias_picker_value)
                    .map(|entry| entry.id),
                Some(descriptor.id)
            );
            assert_eq!(
                runtime_proxy_claude_target_model(alias_picker_value),
                descriptor.id
            );
        }

        if let Some(legacy_picker_model) = descriptor.legacy_claude_picker_model {
            assert!(runtime_proxy_claude_managed_model_option_value(
                legacy_picker_model
            ));
            assert_eq!(
                runtime_proxy_claude_picker_model_descriptor(legacy_picker_model)
                    .map(|entry| entry.id),
                Some(descriptor.id)
            );
            assert_eq!(
                runtime_proxy_claude_target_model(legacy_picker_model),
                descriptor.id
            );
        }
    }
}

#[test]
fn runtime_proxy_claude_reasoning_effort_override_normalizes_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "max");
    assert_eq!(
        runtime_proxy_claude_reasoning_effort_override(),
        Some("xhigh".to_string())
    );
}

#[test]
fn runtime_proxy_claude_reasoning_effort_override_ignores_invalid_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "ultra");
    assert_eq!(runtime_proxy_claude_reasoning_effort_override(), None);
}

#[test]
fn parse_runtime_proxy_claude_version_text_extracts_semver_prefix() {
    assert_eq!(
        parse_runtime_proxy_claude_version_text("2.1.90 (Claude Code)"),
        Some("2.1.90".to_string())
    );
    assert_eq!(
        parse_runtime_proxy_claude_version_text("Claude Code 2.1.90"),
        Some("2.1.90".to_string())
    );
    assert_eq!(parse_runtime_proxy_claude_version_text("Claude Code"), None);
}

#[test]
fn runtime_proxy_serves_local_anthropic_compat_metadata_routes() {
    let temp_dir = TestDir::isolated();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "compat-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([(
            "main".to_string(),
            ProfileEntry {
                codex_home: profile_home,
                managed: true,
                email: Some("main@example.com".to_string()),
                provider: ProfileProvider::Openai,
            },
        )]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let client = Client::builder().build().expect("client");

    let root: serde_json::Value = client
        .get(format!("http://{}/", proxy.listen_addr))
        .send()
        .expect("root request should succeed")
        .json()
        .expect("root response should parse");
    assert_eq!(
        root.get("service").and_then(serde_json::Value::as_str),
        Some("prodex")
    );
    assert_eq!(
        root.get("status").and_then(serde_json::Value::as_str),
        Some("ok")
    );

    let health: serde_json::Value = client
        .get(format!("http://{}/health", proxy.listen_addr))
        .send()
        .expect("health request should succeed")
        .json()
        .expect("health response should parse");
    assert_eq!(
        health.get("status").and_then(serde_json::Value::as_str),
        Some("ok")
    );
    let head = client
        .head(format!("http://{}/", proxy.listen_addr))
        .send()
        .expect("root HEAD request should succeed");
    assert!(
        head.status().is_success(),
        "unexpected HEAD status: {}",
        head.status()
    );

    let models: serde_json::Value = client
        .get(format!("http://{}/v1/models?beta=true", proxy.listen_addr))
        .send()
        .expect("models request should succeed")
        .json()
        .expect("models response should parse");
    let data = models
        .get("data")
        .and_then(serde_json::Value::as_array)
        .expect("models data should be an array");
    assert_eq!(
        data.len(),
        4,
        "metadata route should expose the prodex Claude OpenAI model catalog"
    );
    assert!(data.iter().all(|model| {
        !model
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .starts_with("claude-")
    }));
    assert!(
        data.iter().any(|model| {
            model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5.4")
        })
    );
    assert!(
        data.iter().any(|model| {
            model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5.2")
        })
    );
    assert!(data.iter().any(|model| {
        model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5.4-mini")
    }));

    let model: serde_json::Value = client
        .get(format!(
            "http://{}/v1/models/gpt-5.4?beta=true",
            proxy.listen_addr
        ))
        .send()
        .expect("model request should succeed")
        .json()
        .expect("model response should parse");
    assert_eq!(
        model.get("id").and_then(serde_json::Value::as_str),
        Some("gpt-5.4")
    );
    assert_eq!(
        model
            .get("display_name")
            .and_then(serde_json::Value::as_str),
        Some("GPT-5.4")
    );

    assert!(
        backend.responses_headers().is_empty(),
        "local compatibility routes should not hit the upstream backend"
    );
}
