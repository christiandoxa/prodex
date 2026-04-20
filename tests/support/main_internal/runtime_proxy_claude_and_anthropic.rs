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
    let temp_dir = TestDir::new();
    let _model_guard = TestEnvVarGuard::unset("PRODEX_CLAUDE_MODEL");
    let config_dir = temp_dir.path.join("claude-config");
    let codex_home = temp_dir.path.join("codex-home");
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43123"
            .parse()
            .expect("listen address should parse"),
        &config_dir,
        &codex_home,
        None,
    );
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
        Some("gpt-5".to_string())
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
    assert!(env
        .iter()
        .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION"));
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
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "gpt-5-mini");
    let temp_dir = TestDir::new();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
        None,
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("gpt-5-mini".to_string())
    );
    assert!(env
        .iter()
        .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION"));
}

#[test]
fn runtime_proxy_claude_launch_env_keeps_custom_picker_entry_for_unknown_override() {
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "my-gateway-model");
    let temp_dir = TestDir::new();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
        None,
    );
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
    let temp_dir = TestDir::new();
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
        None,
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("opus".to_string())
    );
    assert!(env
        .iter()
        .all(|(key, _)| *key != "ANTHROPIC_CUSTOM_MODEL_OPTION"));
}

#[test]
fn runtime_proxy_claude_launch_env_maps_alias_backed_override_to_builtin_picker_value() {
    let _model_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_MODEL", "gpt-5.4");
    let temp_dir = TestDir::new();
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
        None,
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "ANTHROPIC_MODEL")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("opus".to_string())
    );
}

#[test]
fn runtime_proxy_claude_launch_env_sets_plugin_root_when_mem_enabled() {
    let temp_dir = TestDir::new();
    let plugin_root = temp_dir.path.join("claude-mem/plugin");
    let env = runtime_proxy_claude_launch_env(
        "127.0.0.1:43124"
            .parse()
            .expect("listen address should parse"),
        &temp_dir.path.join("claude-config"),
        &temp_dir.path.join("codex-home"),
        Some(&plugin_root),
    );
    assert_eq!(
        env.iter()
            .find(|(key, _)| *key == "CLAUDE_PLUGIN_ROOT")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(plugin_root.to_string_lossy().into_owned())
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
        "gpt-5-mini".to_string()
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
fn ensure_runtime_proxy_claude_launch_config_seeds_onboarding_and_project_trust() {
    let temp_dir = TestDir::new();
    let config_dir = temp_dir.path.join("claude-config");
    let cwd = temp_dir.path.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace dir should exist");

    ensure_runtime_proxy_claude_launch_config(&config_dir, &cwd, Some("2.1.90"))
        .expect("Claude config seed should succeed");

    let config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(config_dir.join(".claude.json"))
            .expect("Claude config should be written"),
    )
    .expect("Claude config should be valid JSON");
    assert_eq!(config["numStartups"], serde_json::json!(1));
    assert_eq!(config["hasCompletedOnboarding"], serde_json::json!(true));
    assert!(config.get("skipWebFetchPreflight").is_none());
    assert_eq!(config["lastOnboardingVersion"], serde_json::json!("2.1.90"));
    let additional_model_options = config["additionalModelOptionsCache"]
        .as_array()
        .expect("additional model options cache should be an array");
    assert_eq!(additional_model_options.len(), 6);
    assert!(!additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.4")
    }));
    assert!(additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.2-codex")
            && entry.get("label").and_then(serde_json::Value::as_str) == Some("gpt-5.2-codex")
    }));

    let project_key = cwd.to_string_lossy().into_owned();
    let project = config["projects"]
        .get(project_key.as_str())
        .expect("seeded project entry should exist");
    assert_eq!(project["hasTrustDialogAccepted"], serde_json::json!(true));
    assert_eq!(project["projectOnboardingSeenCount"], serde_json::json!(1));
    assert!(project["allowedTools"].is_array());
    assert_eq!(
        project["allowedTools"],
        serde_json::json!(["WebSearch", "WebFetch"])
    );
    assert!(project["mcpContextUris"].is_array());
    assert!(project["mcpServers"].is_object());
    assert!(project["enabledMcpjsonServers"].is_array());
    assert!(project["disabledMcpjsonServers"].is_array());
    assert!(project["exampleFiles"].is_array());

    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(config_dir.join("settings.json"))
            .expect("Claude settings should be written"),
    )
    .expect("Claude settings should be valid JSON");
    assert_eq!(settings["skipWebFetchPreflight"], serde_json::json!(true));
    assert_eq!(
        settings["permissions"]["allow"],
        serde_json::json!(["WebSearch", "WebFetch"])
    );
}

#[test]
fn ensure_runtime_proxy_claude_launch_config_preserves_existing_entries() {
    let temp_dir = TestDir::new();
    let config_dir = temp_dir.path.join("claude-config");
    let cwd = temp_dir.path.join("workspace");
    let other_project = temp_dir.path.join("other");
    fs::create_dir_all(&cwd).expect("workspace dir should exist");
    fs::create_dir_all(&config_dir).expect("config dir should exist");
    fs::write(
        config_dir.join(".claude.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "numStartups": 7,
            "customField": "keep-me",
            "additionalModelOptionsCache": [
                {
                    "value": "claude-opus-4-6",
                    "label": "gpt-5.4",
                    "description": "Old managed model entry"
                },
                {
                    "value": "custom-provider/model",
                    "label": "custom-provider/model",
                    "description": "Existing custom model"
                }
            ],
            "projects": {
                other_project.to_string_lossy().to_string(): {
                    "hasTrustDialogAccepted": false,
                    "allowedTools": ["Bash"]
                }
            }
        }))
        .expect("existing config should serialize"),
    )
    .expect("existing config should write");
    fs::write(
        config_dir.join("settings.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "effortLevel": "high",
            "customSetting": "keep-me",
            "skipWebFetchPreflight": false,
            "permissions": {
                "allow": ["Bash", "WebSearch"]
            }
        }))
        .expect("existing settings should serialize"),
    )
    .expect("existing settings should write");

    ensure_runtime_proxy_claude_launch_config(&config_dir, &cwd, Some("2.1.90"))
        .expect("Claude config merge should succeed");

    let config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(config_dir.join(".claude.json"))
            .expect("Claude config should still exist"),
    )
    .expect("merged Claude config should be valid JSON");
    assert_eq!(config["numStartups"], serde_json::json!(7));
    assert_eq!(config["customField"], serde_json::json!("keep-me"));
    assert!(config.get("skipWebFetchPreflight").is_none());
    let additional_model_options = config["additionalModelOptionsCache"]
        .as_array()
        .expect("additional model options cache should be preserved as an array");
    assert!(additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("custom-provider/model")
    }));
    assert!(additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.2")
            && entry.get("label").and_then(serde_json::Value::as_str) == Some("gpt-5.2")
            && entry.get("supportedEffortLevels")
                == Some(&serde_json::json!(["low", "medium", "high", "max"]))
    }));
    assert!(!additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.4")
    }));
    assert!(!additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("claude-opus-4-6")
    }));
    let other_project_key = other_project.to_string_lossy().to_string();
    let cwd_key = cwd.to_string_lossy().to_string();
    assert_eq!(
        config["projects"][other_project_key.as_str()]["allowedTools"],
        serde_json::json!(["Bash"])
    );
    assert_eq!(
        config["projects"][cwd_key.as_str()]["hasTrustDialogAccepted"],
        serde_json::json!(true)
    );
    assert_eq!(
        config["projects"][cwd_key.as_str()]["allowedTools"],
        serde_json::json!(["WebSearch", "WebFetch"])
    );

    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(config_dir.join("settings.json"))
            .expect("Claude settings should still exist"),
    )
    .expect("merged Claude settings should be valid JSON");
    assert_eq!(settings["effortLevel"], serde_json::json!("high"));
    assert_eq!(settings["customSetting"], serde_json::json!("keep-me"));
    assert_eq!(settings["skipWebFetchPreflight"], serde_json::json!(true));
    assert_eq!(
        settings["permissions"]["allow"],
        serde_json::json!(["Bash", "WebSearch", "WebFetch"])
    );
}

#[test]
fn ensure_runtime_proxy_claude_launch_config_appends_default_web_tools_to_current_project() {
    let temp_dir = TestDir::new();
    let config_dir = temp_dir.path.join("claude-config");
    let cwd = temp_dir.path.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace dir should exist");
    fs::create_dir_all(&config_dir).expect("config dir should exist");
    fs::write(
        config_dir.join(".claude.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "projects": {
                cwd.to_string_lossy().to_string(): {
                    "allowedTools": ["Bash", "WebSearch"]
                }
            }
        }))
        .expect("existing config should serialize"),
    )
    .expect("existing config should write");

    ensure_runtime_proxy_claude_launch_config(&config_dir, &cwd, Some("2.1.90"))
        .expect("Claude config merge should succeed");

    let config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(config_dir.join(".claude.json"))
            .expect("Claude config should still exist"),
    )
    .expect("merged Claude config should be valid JSON");
    assert_eq!(
        config["projects"][cwd.to_string_lossy().as_ref()]["allowedTools"],
        serde_json::json!(["Bash", "WebSearch", "WebFetch"])
    );
}

#[test]
fn prepare_runtime_proxy_claude_config_dir_imports_legacy_home_into_shared_state() {
    let temp_dir = TestDir::new();
    let home_dir = temp_dir.path.join("home");
    let legacy_claude_dir = home_dir.join(".claude");
    let legacy_project_dir = legacy_claude_dir.join("projects/workspace");
    fs::create_dir_all(&legacy_project_dir).expect("legacy Claude project dir should exist");
    fs::write(
        legacy_project_dir.join("session.jsonl"),
        "{\"message\":\"first\"}\n{\"message\":\"second\"}\n",
    )
    .expect("legacy Claude chat history should write");
    fs::write(
        legacy_claude_dir.join("settings.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "effortLevel": "high"
        }))
        .expect("legacy Claude settings should serialize"),
    )
    .expect("legacy Claude settings should write");
    fs::write(
        home_dir.join(".claude.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "customField": "keep-me",
            "projects": {
                "/tmp/legacy": {
                    "allowedTools": ["Bash"]
                }
            }
        }))
        .expect("legacy Claude config should serialize"),
    )
    .expect("legacy Claude config should write");
    let _home_guard =
        TestEnvVarGuard::set("HOME", home_dir.to_str().expect("home should be utf-8"));

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let codex_home = paths.managed_profiles_root.join("main");

    let config_dir = prepare_runtime_proxy_claude_config_dir(&paths, &codex_home, true)
        .expect("Claude config dir should be prepared");
    let shared_dir = runtime_proxy_shared_claude_config_dir(&paths);
    assert!(
        same_path(&config_dir, &shared_dir),
        "managed Claude config dir should point at shared Prodex Claude state"
    );
    assert!(
        runtime_proxy_claude_legacy_import_marker_path(&shared_dir).exists(),
        "legacy import marker should be written after successful import"
    );
    assert_eq!(
        fs::read_to_string(shared_dir.join("projects/workspace/session.jsonl"))
            .expect("shared Claude history should be readable"),
        "{\"message\":\"first\"}\n{\"message\":\"second\"}\n"
    );

    let config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(shared_dir.join(".claude.json"))
            .expect("shared Claude config should be readable"),
    )
    .expect("shared Claude config should parse");
    assert_eq!(config["customField"], serde_json::json!("keep-me"));
    assert_eq!(
        config["projects"]["/tmp/legacy"]["allowedTools"],
        serde_json::json!(["Bash"])
    );
    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(shared_dir.join("settings.json"))
            .expect("shared Claude settings should be readable"),
    )
    .expect("shared Claude settings should parse");
    assert_eq!(settings["effortLevel"], serde_json::json!("high"));
}

#[test]
fn prepare_runtime_proxy_claude_config_dir_migrates_existing_profile_state_into_shared_root() {
    let temp_dir = TestDir::new();
    let home_dir = temp_dir.path.join("home");
    fs::create_dir_all(&home_dir).expect("home dir should exist");
    let _home_guard =
        TestEnvVarGuard::set("HOME", home_dir.to_str().expect("home should be utf-8"));

    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let codex_home = paths.managed_profiles_root.join("main");
    let profile_dir = runtime_proxy_claude_config_dir(&codex_home);
    let shared_dir = runtime_proxy_shared_claude_config_dir(&paths);

    fs::create_dir_all(profile_dir.join("projects/workspace"))
        .expect("legacy profile Claude project dir should exist");
    fs::write(
        profile_dir.join("projects/workspace/session.jsonl"),
        "{\"message\":\"first\"}\n{\"message\":\"second\"}\n",
    )
    .expect("legacy profile Claude history should write");
    fs::write(
        profile_dir.join(".claude.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "profileOnly": true
        }))
        .expect("legacy profile Claude config should serialize"),
    )
    .expect("legacy profile Claude config should write");
    fs::write(
        profile_dir.join("settings.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "effortLevel": "high"
        }))
        .expect("legacy profile Claude settings should serialize"),
    )
    .expect("legacy profile Claude settings should write");

    fs::create_dir_all(shared_dir.join("projects/workspace"))
        .expect("shared Claude project dir should exist");
    fs::write(
        shared_dir.join("projects/workspace/session.jsonl"),
        "{\"message\":\"first\"}\n",
    )
    .expect("shared Claude history should write");
    fs::write(
        shared_dir.join(".claude.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "sharedOnly": true
        }))
        .expect("shared Claude config should serialize"),
    )
    .expect("shared Claude config should write");
    fs::write(
        shared_dir.join("settings.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "model": "gpt-5.4"
        }))
        .expect("shared Claude settings should serialize"),
    )
    .expect("shared Claude settings should write");

    let config_dir = prepare_runtime_proxy_claude_config_dir(&paths, &codex_home, true)
        .expect("Claude config dir should be prepared");
    assert!(
        same_path(&config_dir, &shared_dir),
        "managed Claude config dir should resolve to shared Prodex Claude state"
    );

    let merged_history = fs::read_to_string(shared_dir.join("projects/workspace/session.jsonl"))
        .expect("merged Claude history should be readable");
    assert_eq!(
        merged_history,
        "{\"message\":\"first\"}\n{\"message\":\"second\"}"
    );
    let config: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(shared_dir.join(".claude.json"))
            .expect("shared Claude config should still be readable"),
    )
    .expect("shared Claude config should parse");
    assert_eq!(config["sharedOnly"], serde_json::json!(true));
    assert_eq!(config["profileOnly"], serde_json::json!(true));
    let settings: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(shared_dir.join("settings.json"))
            .expect("shared Claude settings should still be readable"),
    )
    .expect("shared Claude settings should parse");
    assert_eq!(settings["model"], serde_json::json!("gpt-5.4"));
    assert_eq!(settings["effortLevel"], serde_json::json!("high"));
}

#[test]
fn runtime_proxy_serves_local_anthropic_compat_metadata_routes() {
    let temp_dir = TestDir::new();
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
        9,
        "metadata route should expose the prodex Claude OpenAI model catalog"
    );
    assert!(data.iter().all(|model| {
        !model
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .starts_with("claude-")
    }));
    assert!(data
        .iter()
        .any(|model| { model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5.4") }));
    assert!(data
        .iter()
        .any(|model| { model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5.2") }));
    assert!(data
        .iter()
        .any(|model| { model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5") }));
    assert!(data.iter().any(|model| {
        model.get("id").and_then(serde_json::Value::as_str) == Some("gpt-5-mini")
    }));

    let model: serde_json::Value = client
        .get(format!(
            "http://{}/v1/models/gpt-5?beta=true",
            proxy.listen_addr
        ))
        .send()
        .expect("model request should succeed")
        .json()
        .expect("model response should parse");
    assert_eq!(
        model.get("id").and_then(serde_json::Value::as_str),
        Some("gpt-5")
    );
    assert_eq!(
        model
            .get("display_name")
            .and_then(serde_json::Value::as_str),
        Some("GPT-5")
    );

    assert!(
        backend.responses_headers().is_empty(),
        "local compatibility routes should not hit the upstream backend"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_tools_and_tool_results() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![
            ("User-Agent".to_string(), "claude-cli/test".to_string()),
            (
                "X-Claude-Code-Session-Id".to_string(),
                "claude-session-123".to_string(),
            ),
        ],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 1024,
            "stream": true,
            "thinking": {
                "type": "adaptive"
            },
            "system": [
                {
                    "type": "text",
                    "text": "System instructions",
                }
            ],
            "output_config": {
                "effort": "medium",
            },
            "tools": [
                {
                    "name": "shell",
                    "description": "Run a shell command",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "tool_choice": {
                "type": "any"
            },
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "Show the logs",
                        },
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "YWJj",
                            }
                        }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Calling shell",
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_1",
                            "name": "shell",
                            "input": {
                                "cmd": "ls"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_1",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "file1",
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    assert_eq!(
        translated.translated_request.path_and_query,
        "/backend-api/codex/responses"
    );
    assert_eq!(translated.requested_model, "claude-sonnet-4-6");
    assert!(translated.stream);
    assert!(translated.want_thinking);
    assert_eq!(
        runtime_proxy_request_header_value(&translated.translated_request.headers, "session_id"),
        Some("claude-session-123")
    );

    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("model").and_then(serde_json::Value::as_str),
        Some("gpt-5.3-codex")
    );
    assert_eq!(
        body.get("instructions").and_then(serde_json::Value::as_str),
        Some("System instructions")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert!(
        body.get("max_tokens").is_none(),
        "translated request should not send unsupported max_tokens"
    );
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("medium")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("parameters"))
            .and_then(|parameters| parameters.get("properties"))
            .and_then(serde_json::Value::as_object)
            .map(serde_json::Map::len),
        Some(0)
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(input.len(), 4);
    assert_eq!(
        input[0].get("role").and_then(serde_json::Value::as_str),
        Some("user")
    );
    assert_eq!(
        input[0]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        input[1].get("role").and_then(serde_json::Value::as_str),
        Some("assistant")
    );
    assert_eq!(
        input[1].get("content").and_then(serde_json::Value::as_str),
        Some("Calling shell")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[2].get("call_id").and_then(serde_json::Value::as_str),
        Some("toolu_1")
    );
    assert_eq!(
        input[3].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );
    assert_eq!(
        input[3].get("output").and_then(serde_json::Value::as_str),
        Some("file1")
    );
}
#[test]
fn translate_runtime_anthropic_messages_request_preserves_tool_references() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_search",
                            "name": "ToolSearch",
                            "input": {
                                "query": "select:WebSearch,WebFetch,TodoWrite"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_search",
                            "content": [
                                {
                                    "type": "tool_reference",
                                    "tool_name": "WebSearch",
                                },
                                {
                                    "type": "tool_reference",
                                    "tool_name": "WebFetch",
                                },
                                {
                                    "type": "tool_reference",
                                    "tool_name": "TodoWrite",
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("ToolSearch")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    let output: serde_json::Value =
        serde_json::from_str(output).expect("tool result output should be valid JSON");
    assert_eq!(
        output
            .get("tool_references")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec!["WebSearch", "WebFetch", "TodoWrite"])
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_upstream_streaming_for_non_stream_client_request(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "stream": false,
            "messages": [
                {
                    "role": "user",
                    "content": "Cari halaman utama openai.com"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    assert!(
        !translated.stream,
        "Anthropic client request should remain buffered locally"
    );

    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("stream").and_then(serde_json::Value::as_bool),
        Some(true),
        "Responses upstream must stay streaming for server-tool follow-up compatibility"
    );
}

#[test]
fn runtime_request_for_anthropic_server_tool_followup_defaults_stream_to_true() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.3-codex",
            "input": [
                {
                    "role": "user",
                    "content": "Cari halaman utama openai.com"
                }
            ],
            "tool_choice": "auto"
        })
        .to_string()
        .into_bytes(),
    };

    let followup =
        runtime_request_for_anthropic_server_tool_followup(&request, "resp_followup_123")
            .expect("follow-up request should serialize");
    let body: serde_json::Value =
        serde_json::from_slice(&followup.body).expect("follow-up body should parse");

    assert_eq!(
        body.get("previous_response_id")
            .and_then(serde_json::Value::as_str),
        Some("resp_followup_123")
    );
    assert!(
        body.get("input").is_none(),
        "follow-up request should omit original input"
    );
    assert!(
        body.get("tool_choice").is_none(),
        "follow-up request should omit tool choice"
    );
    assert_eq!(
        body.get("stream").and_then(serde_json::Value::as_bool),
        Some(true),
        "follow-up request must request a streaming Responses transport"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_document_content_to_input_text() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "document",
                            "source": {
                                "type": "text",
                                "media_type": "text/plain",
                                "data": "Document body",
                            }
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("role").and_then(serde_json::Value::as_str),
        Some("user")
    );
    assert_eq!(
        input[0].get("content").and_then(serde_json::Value::as_str),
        Some("Document body")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_web_fetch_tool_result() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "server_tool_use",
                            "id": "srvtoolu_fetch",
                            "name": "web_fetch",
                            "input": {
                                "url": "https://example.com/article"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "web_fetch_tool_result",
                            "tool_use_id": "srvtoolu_fetch",
                            "content": {
                                "type": "web_fetch_result",
                                "url": "https://example.com/article",
                                "content": {
                                    "type": "document",
                                    "source": {
                                        "type": "text",
                                        "media_type": "text/plain",
                                        "data": "Example content",
                                    },
                                    "title": "Example Article",
                                },
                                "retrieved_at": "2026-04-09T03:00:00Z",
                            }
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    let output: serde_json::Value =
        serde_json::from_str(output).expect("tool result output should be valid JSON");
    assert_eq!(
        output.get("type").and_then(serde_json::Value::as_str),
        Some("web_fetch_result")
    );
    assert_eq!(
        output.get("url").and_then(serde_json::Value::as_str),
        Some("https://example.com/article")
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("Example content")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_code_execution_tool_result() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "server_tool_use",
                            "id": "srvtoolu_code",
                            "name": "bash_code_execution",
                            "input": {
                                "command": "ls -la | head -5"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "bash_code_execution_tool_result",
                            "tool_use_id": "srvtoolu_code",
                            "content": {
                                "type": "bash_code_execution_result",
                                "stdout": "file_a\nfile_b",
                                "stderr": "warning: ignored file",
                                "return_code": 0
                            }
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("bash_code_execution")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    let output: serde_json::Value =
        serde_json::from_str(output).expect("tool result output should be valid JSON");
    assert_eq!(
        output.get("type").and_then(serde_json::Value::as_str),
        Some("bash_code_execution_result")
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("file_a\nfile_b\nwarning: ignored file")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_file_backed_document_and_container_blocks(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "server_tool_use",
                            "id": "srvtoolu_file",
                            "name": "web_fetch",
                            "input": {
                                "url": "https://example.com/file"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "srvtoolu_file",
                            "content": [
                                {
                                    "type": "document",
                                    "source": {
                                        "type": "base64",
                                        "media_type": "text/plain",
                                        "data": "RmlsZSBib2R5"
                                    },
                                    "title": "File payload"
                                },
                                {
                                    "type": "container_upload",
                                    "container_id": "container_123",
                                    "path": "/tmp/prodex/input.txt"
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    let output = input
        .iter()
        .find(|item| {
            item.get("type").and_then(serde_json::Value::as_str) == Some("function_call_output")
        })
        .expect("function_call_output should exist");
    let output: serde_json::Value = serde_json::from_str(
        output
            .get("output")
            .and_then(serde_json::Value::as_str)
            .expect("output should be a string"),
    )
    .expect("output should parse");

    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("File body")
    );
    let content_blocks = output
        .get("content_blocks")
        .and_then(serde_json::Value::as_array)
        .expect("content_blocks should be preserved");
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("document")
            && block
                .get("source")
                .and_then(|source| source.get("type"))
                .and_then(serde_json::Value::as_str)
                == Some("base64")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("container_upload")
            && block
                .get("container_id")
                .and_then(serde_json::Value::as_str)
                == Some("container_123")
    }));
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_generic_server_tool_use_name() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "server_tool_use",
                            "id": "srvtoolu_browser",
                            "name": "browser_fetch",
                            "input": {
                                "url": "https://example.com/article"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": "done"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("call_id").and_then(serde_json::Value::as_str),
        Some("srvtoolu_browser")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("browser_fetch")
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            input[0]
                .get("arguments")
                .and_then(serde_json::Value::as_str)
                .expect("arguments should be a string")
        )
        .expect("arguments should parse"),
        serde_json::json!({
            "url": "https://example.com/article"
        })
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_mcp_tool_use_server_name() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "mcp_tool_use",
                            "id": "mcpu_1",
                            "name": "filesystem",
                            "server_name": "local_fs",
                            "input": {
                                "path": "/tmp/prodex"
                            }
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("call_id").and_then(serde_json::Value::as_str),
        Some("mcpu_1")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("filesystem")
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            input[0]
                .get("arguments")
                .and_then(serde_json::Value::as_str)
                .expect("arguments should be a string")
        )
        .expect("arguments should parse"),
        serde_json::json!({
            "path": "/tmp/prodex",
            "server_name": "local_fs"
        })
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_accepts_mcp_servers_with_mcp_tool_use() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "mcp_servers": [
                {
                    "name": "filesystem",
                    "type": "stdio",
                    "command": "mcp-fs"
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "mcp_tool_use",
                            "id": "mcpu_2",
                            "name": "filesystem",
                            "server_name": "local_fs",
                            "input": {
                                "path": "/tmp/prodex"
                            }
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("call_id").and_then(serde_json::Value::as_str),
        Some("mcpu_2")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("filesystem")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_mcp_toolset_to_responses_mcp() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "mcp_servers": [
                {
                    "name": "local_fs",
                    "type": "url",
                    "url": "https://mcp.example.com/sse",
                    "authorization_token": "token_123"
                }
            ],
            "tools": [
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "local_fs",
                    "description": "Local filesystem tools",
                    "default_config": {
                        "enabled": false,
                        "defer_loading": true
                    },
                    "configs": {
                        "read_file": {
                            "enabled": true
                        }
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Read the project file"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("tools array should exist");

    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp")
    );
    assert_eq!(
        tools[0]
            .get("server_label")
            .and_then(serde_json::Value::as_str),
        Some("local_fs")
    );
    assert_eq!(
        tools[0]
            .get("server_url")
            .and_then(serde_json::Value::as_str),
        Some("https://mcp.example.com/sse")
    );
    assert_eq!(
        tools[0]
            .get("authorization")
            .and_then(serde_json::Value::as_str),
        Some("token_123")
    );
    assert_eq!(
        tools[0]
            .get("require_approval")
            .and_then(serde_json::Value::as_str),
        Some("never")
    );
    assert_eq!(
        tools[0]
            .get("defer_loading")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        tools[0].get("allowed_tools"),
        Some(&serde_json::json!(["read_file"]))
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_does_not_buffer_mcp_only_toolsets() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), "dummy".to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "stream": true,
            "mcp_servers": [
                {
                    "name": "local_fs",
                    "url": "https://mcp.example.com/sse"
                }
            ],
            "tools": [
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "local_fs",
                    "name": "filesystem"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "List the workspace files."
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    assert!(
        !translated.server_tools.needs_buffered_translation(),
        "mcp-only anthropic requests should stay on the streaming path"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_falls_back_for_unrepresentable_mcp_toolset_denylist(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "mcp_servers": [
                {
                    "name": "local_fs",
                    "type": "url",
                    "url": "https://mcp.example.com/sse"
                }
            ],
            "tools": [
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "local_fs",
                    "configs": {
                        "delete_file": {
                            "enabled": false
                        }
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the workspace"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("tools array should exist");

    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0].get("name").and_then(serde_json::Value::as_str),
        Some("mcp_toolset")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_unsupported_user_content_blocks() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "document",
                            "source": {
                                "type": "file",
                                "file_id": "file_123",
                                "filename": "report.pdf"
                            }
                        },
                        {
                            "type": "container_upload",
                            "container_id": "container_123",
                            "path": "/tmp/prodex/report.pdf"
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 1);
    let content = input[0]
        .get("content")
        .and_then(serde_json::Value::as_str)
        .expect("content should be a string fallback");
    assert!(content.contains("[anthropic:document]"));
    assert!(content.contains("\"file_id\":\"file_123\""));
    assert!(content.contains("[anthropic:container_upload]"));
}

#[test]
fn translate_runtime_anthropic_messages_request_roundtrips_search_result_content_blocks() {
    let search_result = serde_json::json!({
        "type": "search_result",
        "source": "https://example.com/search?q=prodex",
        "title": "Prodex Search Hit",
        "snippet": "Prodex wraps codex and manages isolated profiles."
    });
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_search",
                            "name": "LocalSearch",
                            "input": {
                                "query": "prodex"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_search",
                            "content": [
                                search_result.clone()
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("LocalSearch")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    let output: serde_json::Value =
        serde_json::from_str(output).expect("tool result output should be valid JSON");

    assert!(
        output.get("text").is_none(),
        "structured-only search results should not be flattened into synthetic text"
    );
    assert_eq!(
        output.get("content_blocks"),
        Some(&serde_json::json!([search_result]))
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_roundtrips_structured_tool_result_content_blocks() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "server_tool_use",
                            "id": "srvtoolu_browser",
                            "name": "browser_fetch",
                            "input": {
                                "url": "https://example.com/article"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "srvtoolu_browser",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Fetched body"
                                },
                                {
                                    "type": "tool_reference",
                                    "tool_name": "browser_fetch"
                                },
                                {
                                    "type": "document",
                                    "source": {
                                        "type": "text",
                                        "media_type": "text/plain",
                                        "data": "Document text"
                                    },
                                    "title": "Doc title"
                                },
                                {
                                    "type": "web_fetch_result",
                                    "url": "https://example.com/article",
                                    "content": {
                                        "type": "document",
                                        "source": {
                                            "type": "text",
                                            "media_type": "text/plain",
                                            "data": "Inner fetch body"
                                        },
                                        "title": "Inner title"
                                    },
                                    "retrieved_at": "2026-04-09T03:00:00Z"
                                },
                                {
                                    "type": "custom_block",
                                    "details": "keep me"
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("browser_fetch")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    let output: serde_json::Value =
        serde_json::from_str(output).expect("tool result output should be valid JSON");

    assert_eq!(
        output
            .get("tool_references")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec!["browser_fetch"])
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("Fetched body\nDocument text\nInner fetch body")
    );

    let content_blocks = output
        .get("content_blocks")
        .and_then(serde_json::Value::as_array)
        .expect("content_blocks should be preserved");
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("text")
            && block.get("text").and_then(serde_json::Value::as_str) == Some("Fetched body")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("tool_reference")
            && block.get("tool_name").and_then(serde_json::Value::as_str) == Some("browser_fetch")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("document")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("web_fetch_result")
    }));
    assert!(content_blocks.iter().any(|block| {
        block.get("type").and_then(serde_json::Value::as_str) == Some("custom_block")
    }));
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_structured_error_tool_result_content() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_text_editor",
                            "name": "text_editor_20250124",
                            "input": {
                                "path": "/tmp/prodex/notes.txt",
                                "old_string": "foo",
                                "new_string": "bar"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_text_editor",
                            "is_error": true,
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Replacement failed."
                                },
                                {
                                    "type": "tool_reference",
                                    "tool_name": "str_replace_based_edit_tool"
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    let output: serde_json::Value =
        serde_json::from_str(output).expect("tool result output should remain valid JSON");

    assert_eq!(
        output.get("is_error").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some("Replacement failed.")
    );
    assert_eq!(
        output
            .get("tool_references")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec!["str_replace_based_edit_tool"])
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_versioned_builtin_client_tools() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                },
                {
                    "type": "text_editor_20250124",
                    "description": "Edit local files"
                },
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900,
                    "display_number": 1
                },
                {
                    "type": "memory_20250818",
                    "description": "Persist project memory"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Open the browser and inspect the page"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");

    assert_eq!(tools.len(), 4);
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("function"),
            Some("function"),
            Some("function"),
            Some("function"),
        ]
    );
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("bash"),
            Some("str_replace_based_edit_tool"),
            Some("computer"),
            Some("memory"),
        ]
    );
    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Run shell commands")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Edit local files")
    );
    assert_eq!(
        tools[2]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Interact with the graphical computer display. Display resolution: 1440x900 pixels. Display number: 1."
        )
    );
    assert_eq!(
        tools[3]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Persist project memory")
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("old_str"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert!(tools[1]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("command"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("undo_edit"))));
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[2]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("action"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[2]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("coordinate"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
    assert_eq!(
        tools[3]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert!(tools[3]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("command"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("rename"))));
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_appends_computer_display_context_to_existing_description(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "computer_20250124",
                    "description": "Inspect the browser UI.",
                    "display_width_px": 1280,
                    "display_height_px": 720,
                    "display_number": 2
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the page"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let description = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .and_then(|tools| tools.first())
        .and_then(|tool| tool.get("description"))
        .and_then(serde_json::Value::as_str);

    assert_eq!(
        description,
        Some(
            "Inspect the browser UI.\n\nInteract with the graphical computer display. Display resolution: 1280x720 pixels. Display number: 2."
        )
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_modern_builtin_tool_capabilities() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "text_editor_20250728",
                    "description": "Edit project files.",
                    "max_characters": 8192
                },
                {
                    "type": "computer_20251124",
                    "description": "Inspect the browser UI.",
                    "display_width_px": 1600,
                    "display_height_px": 900,
                    "display_number": 1,
                    "enable_zoom": true
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the project state"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");

    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Edit project files.\n\nEdit local text files with string replacement operations. View results may be truncated to 8192 characters."
        )
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|property| property.get("enum"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec!["view", "create", "str_replace", "insert"])
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Inspect the browser UI.\n\nInteract with the graphical computer display. Display resolution: 1600x900 pixels. Display number: 1. Zoom action enabled."
        )
    );
    assert!(tools[1]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("action"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
        })
        .is_some_and(|values| values.contains(&"zoom")));
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("region"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_modern_builtin_client_tool_capabilities()
{
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tools": [
                {
                    "type": "text_editor_20250728",
                    "name": "str_replace_based_edit_tool",
                    "max_characters": 10000
                },
                {
                    "type": "computer_20251124",
                    "display_width_px": 1600,
                    "display_height_px": 900,
                    "display_number": 1,
                    "enable_zoom": true
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect and edit the project files."
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");

    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Edit local text files with string replacement operations. View results may be truncated to 10000 characters."
        )
    );
    assert!(!tools[0]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("command"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values
            .iter()
            .any(|value| value.as_str() == Some("undo_edit"))));
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("insert_text"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("string")
    );
    assert_eq!(
        tools[1]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some(
            "Interact with the graphical computer display. Display resolution: 1600x900 pixels. Display number: 1. Zoom action enabled."
        )
    );
    assert!(tools[1]
        .get("parameters")
        .and_then(|parameters| parameters.get("properties"))
        .and_then(|properties| properties.get("action"))
        .and_then(|property| property.get("enum"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("zoom"))));
    assert_eq!(
        tools[1]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("region"))
            .and_then(|property| property.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("array")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_bash_tool_to_native_shell_when_enabled() {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "shell");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "bash_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Running ls."
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_bash",
                            "name": "bash",
                            "input": {
                                "command": "ls -la",
                                "timeout_ms": 1200,
                                "max_output_length": 4096
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_bash",
                            "content": "file1\nfile2"
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("shell")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[0].get("role").and_then(serde_json::Value::as_str),
        Some("assistant")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("shell_call")
    );
    assert_eq!(
        input[1].get("call_id").and_then(serde_json::Value::as_str),
        Some("toolu_bash")
    );
    assert_eq!(
        input[1]
            .get("action")
            .and_then(|action| action.get("commands"))
            .and_then(serde_json::Value::as_array)
            .and_then(|commands| commands.first())
            .and_then(serde_json::Value::as_str),
        Some("ls -la")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("shell_call_output")
    );
    assert_eq!(
        input[2]
            .get("max_output_length")
            .and_then(serde_json::Value::as_u64),
        Some(4096)
    );
    assert_eq!(
        input[2]
            .get("output")
            .and_then(serde_json::Value::as_array)
            .and_then(|output| output.first())
            .and_then(|entry| entry.get("stdout"))
            .and_then(serde_json::Value::as_str),
        Some("file1\nfile2")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_computer_tool_to_native_computer_when_enabled()
{
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "computer");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900,
                    "display_number": 1
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "text",
                            "text": "Capturing the current screen."
                        },
                        {
                            "type": "tool_use",
                            "id": "toolu_computer",
                            "name": "computer",
                            "input": {
                                "action": "screenshot"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_computer",
                            "content": [
                                {
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": "image/png",
                                        "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7+o2kAAAAASUVORK5CYII="
                                    }
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("computer_call")
    );
    assert_eq!(
        input[1]
            .get("actions")
            .and_then(serde_json::Value::as_array)
            .and_then(|actions| actions.first())
            .and_then(|action| action.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("screenshot")
    );
    assert_eq!(
        input[2].get("type").and_then(serde_json::Value::as_str),
        Some("computer_call_output")
    );
    assert_eq!(
        input[2]
            .get("output")
            .and_then(|output| output.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("computer_screenshot")
    );
    assert!(input[2]
        .get("output")
        .and_then(|output| output.get("image_url"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value.starts_with("data:image/png;base64,")));
}

#[test]
fn translate_runtime_anthropic_messages_request_falls_back_for_ambiguous_native_computer_tool_choice(
) {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "computer");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-opus-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "computer_20250124"
            },
            "tools": [
                {
                    "type": "computer_20250124",
                    "display_width_px": 1440,
                    "display_height_px": 900
                },
                {
                    "name": "lookup_logs",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the UI"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(|tools| {
                tools
                    .iter()
                    .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
            }),
        Some(vec![Some("function"), Some("function")])
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_falls_back_for_ambiguous_native_bash_tool_choice() {
    let _guard = TestEnvVarGuard::set("PRODEX_CLAUDE_NATIVE_CLIENT_TOOLS", "shell");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "bash_20250124"
            },
            "tools": [
                {
                    "type": "bash_20250124",
                    "description": "Run shell commands"
                },
                {
                    "name": "lookup_logs",
                    "description": "Read application logs",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Inspect the logs"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![Some("function"), Some("function")]
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("bash")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_normalizes_versioned_client_tool_choice_aliases() {
    let cases = [
        ("bash_20250124", "bash"),
        ("text_editor_20250124", "str_replace_based_edit_tool"),
        ("computer_20250124", "computer"),
        ("memory_20250818", "memory"),
    ];

    for (versioned_tool_type, expected_name) in cases {
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/messages?beta=true".to_string(),
            headers: vec![],
            body: serde_json::json!({
                "model": "claude-sonnet-4-6",
                "tool_choice": {
                    "type": "tool",
                    "name": versioned_tool_type
                },
                "tools": [
                    {
                        "type": versioned_tool_type,
                        "description": "Run a versioned client tool"
                    }
                ],
                "messages": [
                    {
                        "role": "user",
                        "content": "Use the requested tool"
                    }
                ]
            })
            .to_string()
            .into_bytes(),
        };

        let translated = translate_runtime_anthropic_messages_request(&request)
            .expect("translation should succeed");
        let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
            .expect("translated body should parse");

        assert_eq!(
            body.get("tools")
                .and_then(serde_json::Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("type"))
                .and_then(serde_json::Value::as_str),
            Some("function"),
            "tool type should remain a generic function for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tools")
                .and_then(serde_json::Value::as_array)
                .and_then(|tools| tools.first())
                .and_then(|tool| tool.get("name"))
                .and_then(serde_json::Value::as_str),
            Some(expected_name),
            "tool name should normalize for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tool_choice")
                .and_then(|tool_choice| tool_choice.get("type"))
                .and_then(serde_json::Value::as_str),
            Some("function"),
            "tool_choice type should normalize for {versioned_tool_type}"
        );
        assert_eq!(
            body.get("tool_choice")
                .and_then(|tool_choice| tool_choice.get("name"))
                .and_then(serde_json::Value::as_str),
            Some(expected_name),
            "tool_choice name should normalize for {versioned_tool_type}"
        );
    }
}

#[test]
fn translate_runtime_anthropic_messages_request_roundtrips_versioned_text_editor_tool_use_and_result(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "text_editor_20250124"
            },
            "tools": [
                {
                    "type": "text_editor_20250124",
                    "description": "Edit local files"
                }
            ],
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_text_editor",
                            "name": "text_editor_20250124",
                            "input": {
                                "path": "/tmp/prodex/notes.txt",
                                "old_string": "foo",
                                "new_string": "bar"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_text_editor",
                            "content": [
                                {
                                    "type": "text",
                                    "text": "Replaced 1 occurrence."
                                }
                            ]
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("translated tools should exist");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0].get("name").and_then(serde_json::Value::as_str),
        Some("str_replace_based_edit_tool")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("str_replace_based_edit_tool")
    );

    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("function_call")
    );
    assert_eq!(
        input[0].get("name").and_then(serde_json::Value::as_str),
        Some("text_editor_20250124")
    );
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );
    assert_eq!(
        input[1].get("output").and_then(serde_json::Value::as_str),
        Some("Replaced 1 occurrence.")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_versioned_client_tools_as_generic_tool_use()
{
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 11,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_bash",
                    "name": "bash_20250124",
                    "arguments": "{\"command\":\"ls -la\"}"
                },
                {
                    "type": "function_call",
                    "call_id": "call_text_editor",
                    "name": "text_editor_20250124",
                    "arguments": "{\"path\":\"/tmp/prodex/notes.txt\"}"
                },
                {
                    "type": "function_call",
                    "call_id": "call_computer",
                    "name": "computer_20250124",
                    "arguments": "{\"display_width_px\":1440}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");

    assert_eq!(
        content
            .iter()
            .map(|block| block.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![Some("tool_use"), Some("tool_use"), Some("tool_use")]
    );
    assert_eq!(
        content
            .iter()
            .map(|block| block.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("bash_20250124"),
            Some("text_editor_20250124"),
            Some("computer_20250124"),
        ]
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_maps_shell_call_to_bash_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 11,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "shell_call",
                    "call_id": "call_shell",
                    "action": {
                        "commands": ["ls -la"],
                        "timeout_ms": 1200,
                        "max_output_length": 4096
                    }
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("bash")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("ls -la")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("timeout_ms"))
            .and_then(serde_json::Value::as_u64),
        Some(1200)
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("max_output_length"))
            .and_then(serde_json::Value::as_u64),
        Some(4096)
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_joins_shell_call_commands_for_bash_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 9,
                "output_tokens": 4
            },
            "output": [
                {
                    "type": "shell_call",
                    "call_id": "call_shell_multi",
                    "action": {
                        "commands": ["pwd", "ls -la"]
                    }
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("pwd\nls -la")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_maps_computer_call_to_computer_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5
            },
            "output": [
                {
                    "type": "computer_call",
                    "call_id": "call_computer",
                    "actions": [
                        {
                            "type": "click",
                            "button": "left",
                            "x": 321,
                            "y": 654
                        }
                    ]
                }
            ]
        }),
        "claude-opus-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("action"))
            .and_then(serde_json::Value::as_str),
        Some("left_click")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("coordinate"))
            .and_then(serde_json::Value::as_array)
            .map(|coordinates| {
                coordinates
                    .iter()
                    .filter_map(serde_json::Value::as_i64)
                    .collect::<Vec<_>>()
            }),
        Some(vec![321, 654])
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_raw_computer_actions_when_not_lossless() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 8,
                "output_tokens": 4
            },
            "output": [
                {
                    "type": "computer_call",
                    "call_id": "call_computer_raw",
                    "actions": [
                        {
                            "type": "screenshot"
                        },
                        {
                            "type": "click",
                            "button": "left",
                            "x": 10,
                            "y": 20
                        }
                    ]
                }
            ]
        }),
        "claude-opus-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("actions"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_memory_tool_definition_to_builtin_function() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "memory_20250818"
            },
            "tools": [
                {
                    "type": "memory_20250818"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Remember that I prefer concise answers"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .expect("tools array should exist");

    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("type").and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        tools[0].get("name").and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert_eq!(
        tools[0]
            .get("description")
            .and_then(serde_json::Value::as_str),
        Some("Store and retrieve information across conversations using persistent memory files.")
    );
    assert_eq!(
        tools[0]
            .get("parameters")
            .and_then(|parameters| parameters.get("properties"))
            .and_then(|properties| properties.get("command"))
            .and_then(|command| command.get("enum"))
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "view",
            "create",
            "str_replace",
            "insert",
            "delete",
            "rename",
        ])
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert!(
        body.get("instructions")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|instructions| instructions.contains("/memories")),
        "memory tool should append memory guidance to instructions"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_appends_memory_tool_guidance_to_system_instructions(
) {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "system": "System instructions",
            "tools": [
                {
                    "type": "memory_20250818",
                    "name": "memory"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Remember that I prefer concise answers"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let instructions = body
        .get("instructions")
        .and_then(serde_json::Value::as_str)
        .expect("instructions should exist");

    assert!(instructions.contains("System instructions"));
    assert!(instructions.contains("/memories"));
    assert!(instructions.contains("memory"));
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_memory_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 7,
                "output_tokens": 3
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_memory",
                    "name": "memory",
                    "arguments": "{\"command\":\"view\",\"path\":\"/memories\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[0].get("name").and_then(serde_json::Value::as_str),
        Some("memory")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("view")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_code_execution_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "code_execution"
            },
            "tools": [
                {
                    "type": "code_execution_20250825"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Analyze the attached data"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("code_execution")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("code_execution")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_tool_search_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "tool_search_tool_regex"
            },
            "tools": [
                {
                    "type": "tool_search_tool_regex_20251119"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Find tools related to browser fetch"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("tool_search_tool_regex")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("parameters"))
            .and_then(|parameters| parameters.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("object")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("tool_search_tool_regex")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_versioned_web_fetch_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "web_fetch"
            },
            "tools": [
                {
                    "type": "web_fetch_20260409",
                    "description": "Fetch the contents of a web page.",
                    "input_schema": {
                        "type": "object"
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Fetch https://example.com"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_compacts_verbose_web_search_tool_result_text() {
    let raw_output = concat!(
        "Web search results for query: \"berita reksadana terbaru Indonesia April 2026\"\n\n",
        "Links: [{\"url\":\"https://example.com/a\"},{\"url\":\"https://example.com/b\"}]\n\n",
        "Links: [{\"url\":\"https://example.com/b\"},{\"url\":\"https://example.com/c\"}]\n\n",
        "No links found.\n\n",
        "Saya sudah melakukan web search untuk kueri itu. Hasil paling relevan yang saya temukan:\n",
        "1. Contoh hasil pertama\n",
        "Link: https://example.com/a\n\n",
        "Kalau mau, saya bisa lanjutkan dengan rangkuman detail.\n\n",
        "REMINDER: You MUST include the sources above in your response to the user using markdown hyperlinks."
    );
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "call_ws_1",
                            "name": "WebSearch",
                            "input": {
                                "query": "berita reksadana terbaru Indonesia April 2026"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "call_ws_1",
                            "content": raw_output
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 2);
    assert_eq!(
        input[1].get("type").and_then(serde_json::Value::as_str),
        Some("function_call_output")
    );

    let output = input[1]
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("tool result output should be a string");
    assert!(
        output.len() < raw_output.len(),
        "normalized output should be smaller than the raw verbose payload"
    );
    let output: serde_json::Value =
        serde_json::from_str(output).expect("normalized output should be valid JSON");
    assert_eq!(
        output.get("query").and_then(serde_json::Value::as_str),
        Some("berita reksadana terbaru Indonesia April 2026")
    );
    assert_eq!(
        output
            .get("content_blocks")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("url").and_then(serde_json::Value::as_str))
                    .collect::<Vec<_>>()
            }),
        Some(vec![
            "https://example.com/a",
            "https://example.com/b",
            "https://example.com/c"
        ])
    );
    assert_eq!(
        output.get("text").and_then(serde_json::Value::as_str),
        Some(
            "Saya sudah melakukan web search untuk kueri itu. Hasil paling relevan yang saya temukan:\n1. Contoh hasil pertama"
        )
    );
}

#[test]
fn runtime_proxy_anthropic_reasoning_effort_normalizes_output_config_levels() {
    let cases = [
        ("gpt-5.4", "low", Some("low")),
        ("gpt-5.4", "medium", Some("medium")),
        ("gpt-5.4", "high", Some("high")),
        ("gpt-5.4", "max", Some("xhigh")),
        ("gpt-5", "max", Some("high")),
        ("gpt-5.4", "HIGH", Some("high")),
        ("gpt-5.4", "unknown", None),
    ];

    for (target_model, input_effort, expected) in cases {
        let value = serde_json::json!({
            "output_config": {
                "effort": input_effort,
            }
        });
        assert_eq!(
            runtime_proxy_anthropic_reasoning_effort(&value, target_model).as_deref(),
            expected,
            "input effort {input_effort:?} normalized incorrectly for target model {target_model}"
        );
    }
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_max_effort_to_xhigh_for_supported_model() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.4",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_keeps_max_effort_at_high_for_legacy_model() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "max",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("high")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_honors_reasoning_override_env() {
    let _effort_guard = TestEnvVarGuard::set("PRODEX_CLAUDE_REASONING_EFFORT", "xhigh");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "gpt-5.2",
            "thinking": {
                "type": "adaptive"
            },
            "output_config": {
                "effort": "low",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    assert_eq!(
        body.get("reasoning")
            .and_then(|reasoning| reasoning.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("xhigh")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_maps_web_search_server_tool() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "web_search"
            },
            "tools": [
                {
                    "type": "web_search_20260209",
                    "name": "web_search",
                    "allowed_domains": ["example.com"],
                    "user_location": {
                        "type": "approximate",
                        "country": "ID",
                        "city": "Jakarta",
                        "region": "Jakarta",
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Cari berita terbaru"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert_eq!(
        body.get("include")
            .and_then(serde_json::Value::as_array)
            .and_then(|include| include.first())
            .and_then(serde_json::Value::as_str),
        Some("web_search_call.action.sources")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("web_search")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("filters"))
            .and_then(|filters| filters.get("allowed_domains"))
            .and_then(serde_json::Value::as_array)
            .and_then(|domains| domains.first())
            .and_then(serde_json::Value::as_str),
        Some("example.com")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("user_location"))
            .and_then(|location| location.get("country"))
            .and_then(serde_json::Value::as_str),
        Some("ID")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_claude_web_search_tool_name() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "WebSearch"
            },
            "tools": [
                {
                    "name": "WebSearch",
                    "description": "Search the web for current information.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string"
                            },
                            "allowed_domains": {
                                "type": "array",
                                "items": {
                                    "type": "string"
                                }
                            },
                            "blocked_domains": {
                                "type": "array",
                                "items": {
                                    "type": "string"
                                }
                            }
                        },
                        "required": ["query"]
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Cari berita terbaru"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert!(
        body.get("include").is_none(),
        "generic Claude WebSearch tools should not enable Responses web_search include filters"
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert!(
        !translated.server_tools.needs_buffered_translation(),
        "generic Claude WebSearch tools must stay on the direct streaming/function path"
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_claude_web_fetch_tool_name() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "WebFetch"
            },
            "tools": [
                {
                    "name": "WebFetch",
                    "description": "Fetch a web page.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "url": {
                                "type": "string"
                            },
                            "prompt": {
                                "type": "string"
                            }
                        },
                        "required": ["url"]
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Lihat isi https://example.com"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tool_choice")
            .and_then(|tool_choice| tool_choice.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebFetch")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("function")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebFetch")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_forces_implicit_web_search_tool_choice() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "web_search_20250305",
                    "name": "web_search",
                    "allowed_domains": []
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Perform a web search for the latest OpenAI news"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");

    assert_eq!(
        body.get("tool_choice").and_then(serde_json::Value::as_str),
        Some("required")
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        body.get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("web_search")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_tool_use_and_usage() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7,
                "input_tokens_details": {
                    "cached_tokens": 3
                }
            },
            "output": [
                {
                    "type": "reasoning",
                    "summary": [
                        {
                            "type": "summary_text",
                            "text": "Plan first",
                        }
                    ]
                },
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello from prodex",
                        }
                    ]
                },
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "shell",
                    "arguments": "{\"cmd\":\"ls\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        true,
    );

    assert_eq!(
        response.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );
    assert_eq!(
        response.get("model").and_then(serde_json::Value::as_str),
        Some("claude-sonnet-4-6")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        content[1].get("text").and_then(serde_json::Value::as_str),
        Some("Hello from prodex")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        content[2].get("id").and_then(serde_json::Value::as_str),
        Some("call_123")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("cache_read_input_tokens"))
            .and_then(serde_json::Value::as_u64),
        Some(3)
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_web_search_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_123",
                    "name": "WebSearch",
                    "arguments": "{\"query\":\"OpenAI latest news today\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_web_fetch_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_456",
                    "name": "web_fetch",
                    "arguments": "{\"url\":\"https://example.com/article\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_counts_tool_search_tool_use() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_search",
                    "name": "tool_search_tool_regex",
                    "arguments": "{\"query\":\"browser_fetch\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("tool_search_tool_regex")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("tool_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_uses_registered_server_tool_aliases() {
    let mut server_tools = RuntimeAnthropicServerTools::default();
    server_tools.register("browser_fetch", "web_fetch");
    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_alias",
                    "name": "browser_fetch",
                    "arguments": "{\"url\":\"https://example.com/article\"}"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
        0,
        0,
        0,
        0,
        Some(&server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("web_fetch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_claude_web_fetch_tool_use() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "WebFetch"
            },
            "tools": [
                {
                    "name": "WebFetch",
                    "description": "Fetch a web page.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "url": {
                                "type": "string"
                            },
                            "prompt": {
                                "type": "string"
                            }
                        },
                        "required": ["url"]
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Lihat isi https://example.com"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_webfetch",
                    "name": "WebFetch",
                    "arguments": "{\"url\":\"https://example.com\",\"prompt\":\"Summarize the page\"}"
                }
            ]
        }),
        &translated.requested_model,
        translated.want_thinking,
        translated.carried_web_search_requests,
        translated.carried_web_fetch_requests,
        translated.carried_code_execution_requests,
        translated.carried_tool_search_requests,
        Some(&translated.server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebFetch")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://example.com")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_claude_web_search_tool_use() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tool_choice": {
                "type": "tool",
                "name": "WebSearch"
            },
            "tools": [
                {
                    "name": "WebSearch",
                    "description": "Search the web for current information.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string"
                            },
                            "allowed_domains": {
                                "type": "array",
                                "items": {
                                    "type": "string"
                                }
                            }
                        },
                        "required": ["query"]
                    }
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Cari berita terbaru"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_websearch",
                    "name": "WebSearch",
                    "arguments": "{\"query\":\"OpenAI latest news today\",\"allowed_domains\":[\"openai.com\"]}"
                }
            ]
        }),
        &translated.requested_model,
        translated.want_thinking,
        translated.carried_web_search_requests,
        translated.carried_web_fetch_requests,
        translated.carried_code_execution_requests,
        translated.carried_tool_search_requests,
        Some(&translated.server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("query"))
            .and_then(serde_json::Value::as_str),
        Some("OpenAI latest news today")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_generic_server_tool_use_from_request_state()
{
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "server_tool_use",
                            "id": "srvtoolu_browser",
                            "name": "browser_fetch",
                            "input": {
                                "url": "https://example.com/article"
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": "done"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "srvtoolu_browser",
                    "name": "browser_fetch",
                    "arguments": "{\"url\":\"https://example.com/article\"}"
                }
            ]
        }),
        &translated.requested_model,
        translated.want_thinking,
        translated.carried_web_search_requests,
        translated.carried_web_fetch_requests,
        translated.carried_code_execution_requests,
        translated.carried_tool_search_requests,
        Some(&translated.server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("browser_fetch")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_code_execution_server_tool_use() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "tools": [
                {
                    "type": "code_execution_20250825"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "Analyze the attached data"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "function_call",
                    "call_id": "srvtoolu_code",
                    "name": "code_execution",
                    "arguments": "{\"code\":\"print(1 + 1)\"}"
                }
            ]
        }),
        &translated.requested_model,
        translated.want_thinking,
        translated.carried_web_search_requests,
        translated.carried_web_fetch_requests,
        translated.carried_code_execution_requests,
        translated.carried_tool_search_requests,
        Some(&translated.server_tools),
    );

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("code_execution")
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_maps_mcp_call_to_blocks() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "mcp_call",
                    "id": "mcp_1",
                    "name": "read_file",
                    "server_label": "local_fs",
                    "arguments": "{\"path\":\"/tmp/prodex/AGENTS.md\"}",
                    "output": "AGENTS content"
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_tool_use")
    );
    assert_eq!(
        content[0]
            .get("server_name")
            .and_then(serde_json::Value::as_str),
        Some("local_fs")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("path"))
            .and_then(serde_json::Value::as_str),
        Some("/tmp/prodex/AGENTS.md")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_tool_result")
    );
    assert_eq!(
        content[1]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("text"))
            .and_then(serde_json::Value::as_str),
        Some("AGENTS content")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("end_turn")
    );
}

#[test]
fn translate_runtime_anthropic_messages_request_preserves_mcp_approval_response_block() {
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages?beta=true".to_string(),
        headers: vec![],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "mcp_approval_response",
                            "approval_request_id": "mcpapr_1",
                            "approve": true,
                            "reason": "Looks good"
                        }
                    ]
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let translated =
        translate_runtime_anthropic_messages_request(&request).expect("translation should succeed");
    let body: serde_json::Value = serde_json::from_slice(&translated.translated_request.body)
        .expect("translated body should parse");
    let input = body
        .get("input")
        .and_then(serde_json::Value::as_array)
        .expect("input array should exist");

    assert_eq!(input.len(), 1);
    assert_eq!(
        input[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_approval_response")
    );
    assert_eq!(
        input[0]
            .get("approval_request_id")
            .and_then(serde_json::Value::as_str),
        Some("mcpapr_1")
    );
    assert_eq!(
        input[0].get("approve").and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_mcp_approval_request_and_list_tools() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 7,
                "output_tokens": 3
            },
            "output": [
                {
                    "type": "mcp_approval_request",
                    "id": "mcpapr_1",
                    "name": "read_file",
                    "server_label": "local_fs",
                    "arguments": "{\"path\":\"/tmp/prodex/AGENTS.md\"}"
                },
                {
                    "type": "mcp_list_tools",
                    "id": "mcplist_1",
                    "server_label": "local_fs",
                    "tools": [
                        {
                            "name": "read_file",
                            "description": "Read a file",
                            "input_schema": {
                                "type": "object",
                                "properties": {
                                    "path": {
                                        "type": "string"
                                    }
                                }
                            }
                        }
                    ]
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_approval_request")
    );
    assert_eq!(
        content[0].get("id").and_then(serde_json::Value::as_str),
        Some("mcpapr_1")
    );
    assert_eq!(
        content[0]
            .get("server_label")
            .and_then(serde_json::Value::as_str),
        Some("local_fs")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("mcp_list_tools")
    );
    assert_eq!(
        content[1]
            .get("tools")
            .and_then(serde_json::Value::as_array)
            .and_then(|tools| tools.first())
            .and_then(|tool| tool.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("read_file")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_preserves_mcp_approval_and_list_tools() {
    let message = serde_json::json!({
        "id": "msg_mcp_adv",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "mcp_approval_request",
                "id": "mcpapr_2",
                "name": "delete_file",
                "server_label": "local_fs",
                "arguments": "{\"path\":\"/tmp/prodex/old.txt\"}"
            },
            {
                "type": "mcp_list_tools",
                "id": "mcplist_2",
                "server_label": "local_fs",
                "tools": [
                    {
                        "name": "delete_file",
                        "description": "Delete a file",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "path": {
                                    "type": "string"
                                }
                            }
                        }
                    }
                ]
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 4,
            "output_tokens": 2,
            "server_tool_use": {
                "web_search_requests": 0,
                "web_fetch_requests": 0
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"mcp_approval_request\""));
    assert!(body.contains("\"mcp_list_tools\""));
    assert!(body.contains("\"mcpapr_2\""));
    assert!(body.contains("\"mcplist_2\""));
}

#[test]
fn runtime_anthropic_response_from_json_value_preserves_web_search_results() {
    let response = runtime_anthropic_response_from_json_value(
        &serde_json::json!({
            "usage": {
                "input_tokens": 12,
                "output_tokens": 7
            },
            "output": [
                {
                    "type": "web_search_call",
                    "id": "ws_123",
                    "status": "completed",
                    "action": {
                        "type": "search",
                        "queries": ["berita teknologi terbaru"],
                        "sources": [
                            {
                                "type": "url",
                                "url": "https://example.com/story"
                            }
                        ]
                    }
                },
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Ringkasan singkat.",
                            "annotations": [
                                {
                                    "type": "url_citation",
                                    "url": "https://example.com/story",
                                    "title": "Example Story"
                                }
                            ]
                        }
                    ]
                }
            ]
        }),
        "claude-sonnet-4-6",
        false,
    );

    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        content[0]
            .get("input")
            .and_then(|input| input.get("query"))
            .and_then(serde_json::Value::as_str),
        Some("berita teknologi terbaru")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("web_search_tool_result")
    );
    assert_eq!(
        content[1]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|results| results.first())
            .and_then(|result| result.get("url"))
            .and_then(serde_json::Value::as_str),
        Some("https://example.com/story")
    );
    assert_eq!(
        content[1]
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|results| results.first())
            .and_then(|result| result.get("title"))
            .and_then(serde_json::Value::as_str),
        Some("Example Story")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("text")
    );
    assert_eq!(
        response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str),
        Some("end_turn")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_collects_deltas_and_tool_use() {
    let body = concat!(
        "event: response.reasoning_summary_text.delta\r\n",
        "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"Plan.\"}\r\n",
        "\r\n",
        "event: response.output_text.delta\r\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"Hello\"}\r\n",
        "\r\n",
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"shell\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_1\",\"delta\":\"{\\\"cmd\\\":\\\"ls\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"shell\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4}}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", true)
        .expect("SSE translation should succeed");
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        content[1].get("text").and_then(serde_json::Value::as_str),
        Some("Hello")
    );
    assert_eq!(
        content[2].get("type").and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(serde_json::Value::as_u64),
        Some(9)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_maps_shell_call_to_bash_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"shell_call\",\"call_id\":\"call_shell\"}}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"shell_call\",\"call_id\":\"call_shell\",\"action\":{\"commands\":[\"pwd\",\"ls -la\"],\"timeout_ms\":1200}}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":7,\"output_tokens\":3},\"output\":[{\"type\":\"shell_call\",\"call_id\":\"call_shell\",\"action\":{\"commands\":[\"pwd\",\"ls -la\"],\"timeout_ms\":1200}}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("bash")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("command"))
            .and_then(serde_json::Value::as_str),
        Some("pwd\nls -la")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("timeout_ms"))
            .and_then(serde_json::Value::as_u64),
        Some(1200)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_maps_computer_call_to_computer_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"computer_call\",\"call_id\":\"call_computer\"}}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"computer_call\",\"call_id\":\"call_computer\",\"actions\":[{\"type\":\"move\",\"x\":100,\"y\":200}]}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":6,\"output_tokens\":3},\"output\":[{\"type\":\"computer_call\",\"call_id\":\"call_computer\",\"actions\":[{\"type\":\"move\",\"x\":100,\"y\":200}]}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-opus-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("computer")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("action"))
            .and_then(serde_json::Value::as_str),
        Some("mouse_move")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("input"))
            .and_then(|input| input.get("coordinate"))
            .and_then(serde_json::Value::as_array)
            .map(|coordinates| {
                coordinates
                    .iter()
                    .filter_map(serde_json::Value::as_i64)
                    .collect::<Vec<_>>()
            }),
        Some(vec![100, 200])
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_preserves_web_search_usage() {
    let body = concat!(
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"web_search_call\",\"id\":\"ws_123\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"queries\":[\"OpenAI latest news today\"],\"sources\":[{\"type\":\"url\",\"url\":\"https://openai.com/index/industrial-policy-for-the-intelligence-age\",\"title\":\"Industrial policy for the Intelligence Age\"}]}}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"tool_usage\":{\"web_search\":{\"num_requests\":1}},\"output\":[{\"type\":\"web_search_call\",\"id\":\"ws_123\",\"status\":\"completed\",\"action\":{\"type\":\"search\",\"queries\":[\"OpenAI latest news today\"],\"sources\":[{\"type\":\"url\",\"url\":\"https://openai.com/index/industrial-policy-for-the-intelligence-age\",\"title\":\"Industrial policy for the Intelligence Age\"}]}},{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"OpenAI published a new industrial policy post.\"}]}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");
    let content = response
        .get("content")
        .and_then(serde_json::Value::as_array)
        .expect("content should be an array");
    assert_eq!(
        content[0].get("type").and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        content[1].get("type").and_then(serde_json::Value::as_str),
        Some("web_search_tool_result")
    );
    assert_eq!(
        content[2].get("text").and_then(serde_json::Value::as_str),
        Some("OpenAI published a new industrial policy post.")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_counts_web_search_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"WebSearch\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_1\",\"delta\":\"{\\\"query\\\":\\\"OpenAI latest news today\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"WebSearch\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("tool_use")
    );
    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("name"))
            .and_then(serde_json::Value::as_str),
        Some("WebSearch")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_response_from_sse_bytes_counts_web_fetch_tool_use() {
    let body = concat!(
        "event: response.output_item.added\r\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_2\",\"name\":\"web_fetch\"}}\r\n",
        "\r\n",
        "event: response.function_call_arguments.delta\r\n",
        "data: {\"type\":\"response.function_call_arguments.delta\",\"call_id\":\"call_2\",\"delta\":\"{\\\"url\\\":\\\"https://example.com/article\\\"}\"}\r\n",
        "\r\n",
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_2\",\"name\":\"web_fetch\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let response = runtime_anthropic_response_from_sse_bytes(&body, "claude-sonnet-4-6", false)
        .expect("SSE translation should succeed");

    assert_eq!(
        response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.first())
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("server_tool_use")
    );
    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_fetch_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_proxy_anthropic_carried_server_tool_usage_counts_latest_tool_chain_suffix() {
    let messages = serde_json::json!([
        {
            "role": "user",
            "content": "older prompt"
        },
        {
            "role": "assistant",
            "content": [
                {
                    "type": "server_tool_use",
                    "id": "srvtoolu_old",
                    "name": "web_search",
                    "input": {
                        "query": "older query"
                    }
                }
            ]
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "web_search_tool_result",
                    "tool_use_id": "srvtoolu_old",
                    "content": []
                }
            ]
        },
        {
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Older answer"
                }
            ]
        },
        {
            "role": "assistant",
            "content": [
                {
                    "type": "server_tool_use",
                    "id": "srvtoolu_latest",
                    "name": "web_search",
                    "input": {
                        "query": "latest query"
                    }
                }
            ]
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "web_search_tool_result",
                    "tool_use_id": "srvtoolu_latest",
                    "content": []
                }
            ]
        }
    ]);

    let usage = runtime_proxy_anthropic_carried_server_tool_usage(
        messages.as_array().expect("messages should be an array"),
    );

    assert_eq!(usage.web_search_requests, 1);
    assert_eq!(usage.web_fetch_requests, 0);
    assert_eq!(usage.tool_search_requests, 0);
}

#[test]
fn runtime_anthropic_response_from_json_value_applies_carried_web_search_usage() {
    let value = serde_json::json!({
        "usage": {
            "input_tokens": 9,
            "output_tokens": 4
        },
        "output": [
            {
                "type": "message",
                "content": [
                    {
                        "type": "output_text",
                        "text": "Done."
                    }
                ]
            }
        ]
    });

    let response = runtime_anthropic_response_from_json_value_with_carried_usage(
        &value,
        "claude-sonnet-4-6",
        false,
        1,
        0,
        0,
        0,
        None,
    );

    assert_eq!(
        response
            .get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_seeds_server_tool_use_in_message_start()
{
    let message = serde_json::json!({
        "id": "msg_123",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": "OpenAI posted a new update."
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 9,
            "output_tokens": 4,
            "server_tool_use": {
                "web_search_requests": 1,
                "web_fetch_requests": 0
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"server_tool_use\""));
    assert!(body.contains("\"web_search_requests\":1"));
    assert!(body.contains("\"web_fetch_requests\":0"));
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_preserves_web_fetch_tool_result() {
    let message = serde_json::json!({
        "id": "msg_fetch",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "server_tool_use",
                "id": "srvtoolu_fetch",
                "name": "web_fetch",
                "input": {
                    "url": "https://example.com/article"
                }
            },
            {
                "type": "web_fetch_tool_result",
                "tool_use_id": "srvtoolu_fetch",
                "content": {
                    "type": "web_fetch_result",
                    "url": "https://example.com/article"
                }
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 9,
            "output_tokens": 4,
            "server_tool_use": {
                "web_search_requests": 0,
                "web_fetch_requests": 1
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"web_fetch_tool_result\""));
    assert!(body.contains("https://example.com/article"));
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_preserves_mcp_server_name() {
    let message = serde_json::json!({
        "id": "msg_mcp",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "mcp_tool_use",
                "id": "mcp_1",
                "name": "read_file",
                "server_name": "local_fs",
                "input": {
                    "path": "/tmp/prodex/AGENTS.md"
                }
            },
            {
                "type": "mcp_tool_result",
                "tool_use_id": "mcp_1",
                "content": [
                    {
                        "type": "text",
                        "text": "AGENTS body"
                    }
                ]
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 4,
            "output_tokens": 2,
            "server_tool_use": {
                "web_search_requests": 0,
                "web_fetch_requests": 0
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"mcp_tool_use\""));
    assert!(body.contains("\"server_name\":\"local_fs\""));
    assert!(body.contains("\"mcp_tool_result\""));
}

#[test]
fn runtime_anthropic_sse_response_parts_from_message_value_preserves_generic_tool_result_blocks() {
    let message = serde_json::json!({
        "id": "msg_456",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "bash_code_execution_tool_result",
                "tool_use_id": "srvtoolu_code",
                "content": {
                    "type": "bash_code_execution_result",
                    "stdout": "ok",
                    "stderr": "",
                    "return_code": 0
                }
            }
        ],
        "model": "claude-sonnet-4-6",
        "stop_reason": "end_turn",
        "stop_sequence": serde_json::Value::Null,
        "usage": {
            "input_tokens": 3,
            "output_tokens": 2,
            "server_tool_use": {
                "web_search_requests": 0,
                "web_fetch_requests": 0
            }
        }
    });

    let parts = runtime_anthropic_sse_response_parts_from_message_value(message);
    let body = String::from_utf8(parts.body.into_vec()).expect("sse body should be valid utf-8");
    assert!(body.contains("\"bash_code_execution_tool_result\""));
    assert!(body.contains("\"srvtoolu_code\""));
    assert!(body.contains("\"stdout\":\"ok\""));
}

#[test]
fn runtime_anthropic_sse_response_parts_from_responses_sse_bytes_preserves_carried_server_tool_usage(
) {
    let body = concat!(
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();

    let parts = runtime_anthropic_sse_response_parts_from_responses_sse_bytes(
        &body,
        "claude-sonnet-4-6",
        false,
        1,
        1,
        0,
        1,
        &RuntimeAnthropicServerTools::default(),
    )
    .expect("buffered SSE translation should succeed");
    let body = String::from_utf8(parts.body.into_vec()).expect("SSE body should decode");

    assert!(body.contains("\"web_search_requests\":1"));
    assert!(body.contains("\"web_fetch_requests\":1"));
    assert!(body.contains("\"tool_search_requests\":1"));
}

#[test]
fn runtime_anthropic_sse_reader_emits_web_search_usage_in_message_delta() {
    let upstream = concat!(
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"function_call\",\"call_id\":\"call_1\",\"name\":\"WebSearch\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"tool_usage\":{\"web_search\":{\"num_requests\":1}},\"output\":[]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let mut reader = RuntimeAnthropicSseReader::new(
        Box::new(Cursor::new(upstream)),
        "claude-sonnet-4-6".to_string(),
        false,
        0,
        0,
        0,
        0,
        RuntimeAnthropicServerTools::default(),
    );
    let mut emitted = Vec::new();
    reader
        .read_to_end(&mut emitted)
        .expect("translated SSE stream should read");
    let body = String::from_utf8(emitted).expect("translated SSE body should decode");

    assert!(body.contains("event: message_delta"));
    assert!(body.contains("\"server_tool_use\""));
    assert!(body.contains("\"web_search_requests\":1"));
    assert!(body.contains("\"web_fetch_requests\":0"));
}

#[test]
fn runtime_anthropic_sse_reader_preserves_mcp_tool_result_error_flag() {
    let upstream = concat!(
        "event: response.output_item.done\r\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"mcp_call\",\"id\":\"mcp_1\",\"name\":\"delete_file\",\"server_label\":\"local_fs\",\"arguments\":\"{\\\"path\\\":\\\"/tmp/prodex/old.txt\\\"}\",\"error\":\"permission denied\"}}\r\n",
        "\r\n",
        "event: response.completed\r\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":9,\"output_tokens\":4},\"output\":[{\"type\":\"mcp_call\",\"id\":\"mcp_1\",\"name\":\"delete_file\",\"server_label\":\"local_fs\",\"arguments\":\"{\\\"path\\\":\\\"/tmp/prodex/old.txt\\\"}\",\"error\":\"permission denied\"}]}}\r\n",
        "\r\n"
    )
    .as_bytes()
    .to_vec();
    let mut reader = RuntimeAnthropicSseReader::new(
        Box::new(Cursor::new(upstream)),
        "claude-sonnet-4-6".to_string(),
        false,
        0,
        0,
        0,
        0,
        RuntimeAnthropicServerTools::default(),
    );
    let mut emitted = Vec::new();
    reader
        .read_to_end(&mut emitted)
        .expect("translated SSE stream should read");
    let body = String::from_utf8(emitted).expect("translated SSE body should decode");

    assert!(body.contains("\"mcp_tool_result\""));
    assert!(body.contains("\"permission denied\""));
    assert!(body.contains("\"is_error\":true"));
}

#[test]
fn runtime_proxy_translates_anthropic_messages_to_responses_and_back() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

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
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .header("x-claude-code-session-id", "claude-session-42")
        .header("User-Agent", "claude-cli/test")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "max_tokens": 256,
                "messages": [
                    {
                        "role": "user",
                        "content": "hello from Claude"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );
    let body: serde_json::Value = response.json().expect("anthropic response should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );
    assert_eq!(
        body.get("model").and_then(serde_json::Value::as_str),
        Some("claude-sonnet-4-6")
    );

    let headers = backend.responses_headers();
    let request_headers = headers
        .first()
        .expect("backend should capture translated request headers");
    assert_eq!(
        request_headers.get("session_id").map(String::as_str),
        Some("claude-session-42")
    );
    assert_eq!(
        request_headers.get("user-agent").map(String::as_str),
        Some("claude-cli/test")
    );
    assert_eq!(
        request_headers
            .get("chatgpt-account-id")
            .map(String::as_str),
        Some("second-account")
    );
    assert!(
        !request_headers.contains_key("x-prodex-internal-request-origin"),
        "internal prodex request origin header must not leak upstream"
    );

    let translated_body = backend
        .responses_bodies()
        .into_iter()
        .next()
        .expect("backend should capture translated request body");
    let translated_json: serde_json::Value =
        serde_json::from_str(&translated_body).expect("translated request body should parse");
    assert_eq!(
        translated_json
            .get("model")
            .and_then(serde_json::Value::as_str),
        Some("gpt-5.3-codex")
    );
    assert_eq!(
        translated_json
            .get("input")
            .and_then(serde_json::Value::as_array)
            .and_then(|input| input.first())
            .and_then(|item| item.get("content"))
            .and_then(serde_json::Value::as_str),
        Some("hello from Claude")
    );
}

#[test]
fn runtime_proxy_anthropic_messages_retries_tool_result_transcript_on_another_profile() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_usage_limit_message();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let state = AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            (
                "main".to_string(),
                ProfileEntry {
                    codex_home: main_home.clone(),
                    managed: true,
                    email: Some("main@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
            (
                "second".to_string(),
                ProfileEntry {
                    codex_home: second_home.clone(),
                    managed: true,
                    email: Some("second@example.com".to_string()),
                    provider: ProfileProvider::Openai,
                },
            ),
        ]),
        last_run_selected_at: BTreeMap::new(),
        response_profile_bindings: BTreeMap::new(),
        session_profile_bindings: BTreeMap::new(),
    };
    state.save(&paths).expect("failed to save initial state");

    let proxy = start_runtime_rotation_proxy(&paths, &state, "main", backend.base_url(), false)
        .expect("runtime proxy should start");
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "stream": true,
                "messages": [
                    {
                        "role": "user",
                        "content": "Run pwd"
                    },
                    {
                        "role": "assistant",
                        "content": [
                            {
                                "type": "tool_use",
                                "id": "toolu_1",
                                "name": "shell",
                                "input": {
                                    "cmd": "pwd"
                                }
                            }
                        ]
                    },
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": "toolu_1",
                                "content": "ok"
                            }
                        ]
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert_eq!(
        response.status(),
        reqwest::StatusCode::OK,
        "unexpected anthropic status: {}",
        response.status()
    );
    let body = response
        .text()
        .expect("anthropic stream body should decode");
    assert!(
        body.contains("event: message_start"),
        "unexpected body: {body}"
    );
    assert!(
        body.contains("event: message_stop"),
        "unexpected body: {body}"
    );
    assert!(
        !body.contains("You've hit your usage limit"),
        "fresh anthropic tool-result transcript should rotate instead of surfacing quota: {body}"
    );
    assert_eq!(
        backend.responses_accounts(),
        vec!["main-account".to_string(), "second-account".to_string()]
    );

    let persisted = wait_for_state(&paths, |state| {
        state.active_profile.as_deref() == Some("second")
    });
    assert_eq!(persisted.active_profile.as_deref(), Some("second"));
}

#[test]
fn runtime_proxy_streams_anthropic_messages_from_buffered_responses() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "stream-account");

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
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .header("x-claude-code-session-id", "claude-session-42")
        .header("User-Agent", "claude-cli/test")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "stream": true,
                "messages": [
                    {
                        "role": "user",
                        "content": "hello from Claude"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let body = response.text().expect("stream body should decode");
    assert!(body.contains("event: message_start"));
    assert!(body.contains("event: message_stop"));
}

#[test]
fn runtime_proxy_streams_anthropic_mcp_messages_without_buffering() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_anthropic_mcp_stream();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");
    let usage = usage_with_main_windows(90, 3600, 90, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
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
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: Local::now().timestamp(),
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage.clone()),
                },
            )]),
            profile_usage_snapshots: BTreeMap::from([("main".to_string(), snapshot)]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), "dummy".to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "stream": true,
            "mcp_servers": [
                {
                    "name": "local_fs",
                    "url": "https://mcp.example.com/sse"
                }
            ],
            "tools": [
                {
                    "type": "mcp_toolset",
                    "mcp_server_name": "local_fs",
                    "name": "filesystem"
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": "List the workspace files."
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };

    let response = proxy_runtime_anthropic_messages_request(42, &request, &shared)
        .expect("anthropic mcp request should succeed");

    let RuntimeResponsesReply::Streaming(response) = response else {
        panic!("expected streaming anthropic mcp response");
    };
    let mut body = String::new();
    let mut reader = response.body;
    reader
        .read_to_string(&mut body)
        .expect("streaming anthropic mcp body should read");
    assert!(body.contains("\"mcp_tool_use\""));
    assert!(body.contains("\"local_fs\""));
    assert!(body.contains("\"mcp_tool_result\""));
    assert!(body.contains("\"stop_reason\":\"end_turn\""));
}

#[test]
fn runtime_proxy_continues_anthropic_web_search_server_tool_responses() {
    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_anthropic_web_search_followup();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

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
    let response = Client::builder()
        .build()
        .expect("client")
        .post(format!(
            "http://{}/v1/messages?beta=true",
            proxy.listen_addr
        ))
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .header("User-Agent", "claude-cli/test")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "user",
                        "content": "Cari berita terbaru reksadana Indonesia"
                    }
                ],
                "tools": [
                    {
                        "type": "web_search_20260209",
                        "name": "web_search"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("anthropic proxy request should succeed");

    assert!(
        response.status().is_success(),
        "unexpected status: {}",
        response.status()
    );
    let body: serde_json::Value = response.json().expect("anthropic response should parse");
    assert_eq!(
        body.get("stop_reason").and_then(serde_json::Value::as_str),
        Some("end_turn")
    );
    assert_eq!(
        body.get("content")
            .and_then(serde_json::Value::as_array)
            .and_then(|content| content.last())
            .and_then(|item| item.get("text"))
            .and_then(serde_json::Value::as_str),
        Some("Ringkasan terbaru reksadana Indonesia.")
    );
    assert_eq!(
        body.get("usage")
            .and_then(|usage| usage.get("server_tool_use"))
            .and_then(|server_tool_use| server_tool_use.get("web_search_requests"))
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );

    let requests = backend.responses_bodies();
    assert_eq!(
        requests.len(),
        2,
        "proxy should issue a follow-up continuation"
    );

    let first_request: serde_json::Value =
        serde_json::from_str(&requests[0]).expect("first request should parse");
    assert_eq!(
        first_request
            .get("input")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    let second_request: serde_json::Value =
        serde_json::from_str(&requests[1]).expect("second request should parse");
    assert_eq!(
        second_request
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str),
        Some("resp_ws_followup_1")
    );
    assert!(
        second_request.get("input").is_none(),
        "follow-up request should not replay the original input"
    );
    assert_eq!(
        second_request
            .get("stream")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn runtime_proxy_returns_anthropic_overloaded_error_when_interactive_capacity_is_full() {
    let _limit_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT", "4");
    let _lane_guard = TestEnvVarGuard::set("PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT", "1");
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(1, 1);

    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_slow_stream();
    let paths = AppPaths {
        root: temp_dir.path.join("prodex"),
        state_file: temp_dir.path.join("prodex/state.json"),
        managed_profiles_root: temp_dir.path.join("prodex/profiles"),
        shared_codex_root: temp_dir.path.join("shared"),
        legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
    };
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

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
    let first_url = format!("http://{}/v1/messages", proxy.listen_addr);
    let second_url = first_url.clone();
    let (first_started_tx, first_started_rx) = std::sync::mpsc::channel();
    let first = thread::spawn(move || {
        let client = Client::builder().build().expect("client");
        let response = client
            .post(first_url)
            .header("Content-Type", "application/json")
            .header("x-api-key", "dummy")
            .header("anthropic-version", "2023-06-01")
            .body(
                serde_json::json!({
                    "model": "claude-sonnet-4-6",
                    "stream": true,
                    "messages": [
                        {
                            "role": "user",
                            "content": "hold the interactive slot"
                        }
                    ]
                })
                .to_string(),
            )
            .send()
            .expect("first anthropic request should start");
        assert_eq!(
            response.status(),
            reqwest::StatusCode::OK,
            "first anthropic request should hold the interactive slot"
        );
        first_started_tx
            .send(())
            .expect("first anthropic request should signal readiness");
        thread::sleep(Duration::from_millis(250));
        let body = response
            .text()
            .expect("first anthropic stream should decode");
        assert!(body.contains("event: message_start"));
    });

    first_started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("first anthropic request should start before the overload probe");

    let client = Client::builder().build().expect("client");
    let response = client
        .post(second_url)
        .header("Content-Type", "application/json")
        .header("x-api-key", "dummy")
        .header("anthropic-version", "2023-06-01")
        .body(
            serde_json::json!({
                "model": "claude-sonnet-4-6",
                "messages": [
                    {
                        "role": "user",
                        "content": "second request"
                    }
                ]
            })
            .to_string(),
        )
        .send()
        .expect("second anthropic request should receive an overload response");

    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = response.json().expect("error body should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("error")
    );
    assert_eq!(
        body.get("error")
            .and_then(|error| error.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("overloaded_error")
    );
    assert!(
        body.get("error")
            .and_then(|error| error.get("message"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("temporarily saturated")),
        "unexpected error body: {body}"
    );

    first.join().expect("first request should join");
}

#[test]
fn runtime_proxy_waits_for_anthropic_inflight_relief_then_succeeds() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(20, 250);

    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let usage = usage_with_main_windows(90, 3600, 90, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
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
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: Local::now().timestamp(),
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage.clone()),
                },
            )]),
            profile_usage_snapshots: BTreeMap::from([("main".to_string(), snapshot)]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        drop(inflight_guard);
    });

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/v1/messages".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), "dummy".to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ],
        body: serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [
                {
                    "role": "user",
                    "content": "second request should wait instead of failing"
                }
            ]
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_anthropic_messages_request(42, &request, &shared)
        .expect("anthropic request should complete after inflight relief");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered anthropic response");
    };
    assert_eq!(parts.status, 200, "unexpected status after inflight wait");
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("type").and_then(serde_json::Value::as_str),
        Some("message")
    );

    release.join().expect("release thread should join");

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "interactive inflight wait should be logged"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "interactive inflight wait completion should be logged"
    );
    assert!(
        log.contains("useful=true"),
        "successful Anthropics inflight wait should log useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "successful Anthropics inflight wait should log inflight_release wake source"
    );
}

#[test]
fn runtime_proxy_waits_for_responses_inflight_relief_then_succeeds() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(40, 250);

    let temp_dir = TestDir::new();
    let backend = RuntimeProxyBackend::start_http_buffered_json();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "second-account");

    let usage = usage_with_main_windows(90, 3600, 90, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
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
            },
            upstream_base_url: backend.base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: Local::now().timestamp(),
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage.clone()),
                },
            )]),
            profile_usage_snapshots: BTreeMap::from([("main".to_string(), snapshot)]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        drop(inflight_guard);
    });

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::json!({
            "input": "second request should wait instead of failing"
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_responses_request(43, &request, &shared)
        .expect("responses request should complete after inflight relief");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered responses reply");
    };
    assert_eq!(parts.status, 200, "unexpected status after inflight wait");
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("id").and_then(serde_json::Value::as_str),
        Some("resp-second")
    );

    release.join().expect("release thread should join");

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "responses inflight wait should be logged"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "responses inflight wait completion should be logged"
    );
    assert!(
        log.contains("useful=true"),
        "successful responses inflight wait should log useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "successful responses inflight wait should log inflight_release wake source"
    );
}

#[test]
fn runtime_profile_inflight_relief_wait_returns_immediately_after_prior_release() {
    let temp_dir = TestDir::new();
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_profile_inflight_release_revision(&shared);
    shared
        .lane_admission
        .inflight_release_revision
        .fetch_add(1, Ordering::SeqCst);

    let started_at = Instant::now();
    assert!(wait_for_runtime_profile_inflight_relief_since(
        &shared,
        Duration::from_millis(100),
        observed_revision,
    ));
    assert!(
        started_at.elapsed() < ci_timing_upper_bound_ms(20, 100),
        "release-aware inflight wait should not sleep after the release was already observed"
    );
}

#[test]
fn runtime_profile_inflight_relief_wait_ignores_active_request_release_notify() {
    let temp_dir = TestDir::new();
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_profile_inflight_release_revision(&shared);
    let active_guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("active request slot should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(active_guard);
    });

    assert!(
        !wait_for_runtime_profile_inflight_relief_since(
            &shared,
            Duration::from_millis(100),
            observed_revision,
        ),
        "active-request release notify should not count as inflight relief"
    );

    release
        .join()
        .expect("active-request release thread should join");
    assert_eq!(
        runtime_profile_inflight_release_revision(&shared),
        observed_revision,
        "active-request release should not change inflight release revision"
    );
}

#[test]
fn runtime_probe_refresh_wait_returns_immediately_after_progress_is_observed() {
    let temp_dir = TestDir::new();
    let _shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_probe_refresh_revision();
    note_runtime_probe_refresh_progress();

    let started_at = Instant::now();
    assert!(wait_for_runtime_probe_refresh_since(
        Duration::from_millis(100),
        observed_revision,
    ));
    assert!(
        started_at.elapsed() < ci_timing_upper_bound_ms(20, 100),
        "probe-refresh wait should not sleep after progress was already observed"
    );
}

#[test]
fn runtime_probe_refresh_wait_ignores_lane_release_notify() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let temp_dir = TestDir::new();
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_probe_refresh_revision();
    let active_guard = try_acquire_runtime_proxy_active_request_slot(
        &shared,
        "http",
        "/backend-api/codex/responses",
    )
    .expect("active request slot should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(active_guard);
    });

    assert!(
        !wait_for_runtime_probe_refresh_since(Duration::from_millis(100), observed_revision),
        "lane-release notify should not count as probe-refresh progress"
    );

    release
        .join()
        .expect("active-request release thread should join");
    assert_eq!(
        runtime_probe_refresh_revision(),
        observed_revision,
        "lane-release notify should not change probe refresh revision"
    );
}

#[test]
fn runtime_probe_refresh_apply_waits_for_busy_runtime_state() {
    let _test_guard = crate::acquire_test_runtime_lock();
    let temp_dir = TestDir::new();
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_probe_refresh_revision();
    let runtime_guard = shared.runtime.lock().expect("runtime lock should succeed");
    let apply_shared = shared.clone();
    let apply_thread = thread::spawn(move || {
        apply_runtime_profile_probe_result(
            &apply_shared,
            "main",
            AuthSummary {
                label: "chatgpt".to_string(),
                quota_compatible: true,
            },
            Ok(usage_with_main_windows(90, 3600, 90, 604_800)),
        )
    });
    thread::sleep(Duration::from_millis(20));

    assert_eq!(
        runtime_probe_refresh_revision(),
        observed_revision,
        "probe refresh revision should not advance until the runtime lock is released"
    );
    assert!(
        !wait_for_runtime_probe_refresh_since(Duration::from_millis(20), observed_revision),
        "probe refresh wait should still time out while the apply thread is blocked on runtime state"
    );

    drop(runtime_guard);
    apply_thread
        .join()
        .expect("apply thread should join")
        .expect("probe apply should succeed after the runtime lock is released");

    assert!(
        wait_for_runtime_probe_refresh_since(Duration::from_millis(100), observed_revision),
        "successful probe apply should wake probe-refresh waiters once fresh data lands"
    );
    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        runtime.profile_probe_cache.contains_key("main"),
        "probe apply should update the probe cache after waiting for the runtime lock"
    );
    assert!(
        runtime.profile_usage_snapshots.contains_key("main"),
        "probe apply should update usage snapshots after waiting for the runtime lock"
    );
}

#[test]
fn runtime_probe_inline_execution_logs_context_without_queue_lag() {
    let temp_dir = TestDir::new();
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    execute_runtime_probe_attempt_inline_for_test(
        &shared,
        "main",
        "startup_probe_warmup",
        Err("timeout".to_string()),
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("startup_probe_warmup_error profile=main error=timeout"),
        "inline probe execution should keep the context-specific marker: {log}"
    );
    assert!(
        !log.contains("startup_probe_warmup_error profile=main lag_ms="),
        "inline probe execution should not report queue lag: {log}"
    );
}

#[test]
fn runtime_probe_queued_execution_logs_refresh_marker_with_queue_lag() {
    let temp_dir = TestDir::new();
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState::default(),
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    execute_runtime_probe_attempt_queued_for_test(
        &shared,
        "main",
        Err("timeout".to_string()),
        Duration::from_millis(50),
        Instant::now(),
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("profile_probe_refresh_error profile=main lag_ms="),
        "queued probe execution should keep the queue-aware marker: {log}"
    );
    assert!(
        log.contains("error=timeout"),
        "queued probe execution should preserve the fetch error: {log}"
    );
}

#[test]
fn runtime_probe_refresh_suppresses_nonlocal_upstream_in_tests_and_wakes_waiters() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home.clone(),
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let backlog_before = runtime_probe_refresh_queue_backlog();
    let observed_revision = runtime_probe_refresh_revision();
    schedule_runtime_probe_refresh(&shared, "main", &main_home);

    assert_eq!(
        runtime_probe_refresh_queue_backlog(),
        backlog_before,
        "suppressed nonlocal probe refresh should not add background queue work"
    );
    assert!(
        wait_for_runtime_probe_refresh_since(ci_timing_upper_bound_ms(20, 200), observed_revision),
        "suppressed nonlocal probe refresh should still wake probe-refresh waiters"
    );

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        !runtime.profile_probe_cache.contains_key("main"),
        "suppressed nonlocal probe refresh should not write probe cache state"
    );
    drop(runtime);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("profile_probe_refresh_suppressed profile=main reason=test_nonlocal_upstream"),
        "suppressed nonlocal probe refresh should be logged"
    );
}

#[test]
fn runtime_probe_refresh_nonlocal_upstream_detection_keeps_loopback_exact() {
    assert!(
        runtime_probe_refresh_nonlocal_upstream_for_test("https://chatgpt.com/backend-api"),
        "nonlocal upstreams should stay suppressed in tests"
    );
    assert!(
        runtime_probe_refresh_nonlocal_upstream_for_test(
            "https://localhost.example.com/backend-api"
        ),
        "host matching should stay exact instead of substring-based"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://localhost:1234/backend-api"),
        "localhost loopback should stay allowed in tests"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://127.0.0.1:1234/backend-api"),
        "IPv4 loopback should stay allowed in tests"
    );
    assert!(
        !runtime_probe_refresh_nonlocal_upstream_for_test("http://[::1]:1234/backend-api"),
        "IPv6 loopback should stay allowed in tests"
    );
}

#[test]
fn runtime_probe_refresh_allows_loopback_upstream_in_tests() {
    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("main".to_string()),
                profiles: BTreeMap::from([(
                    "main".to_string(),
                    ProfileEntry {
                        codex_home: main_home.clone(),
                        managed: true,
                        email: Some("main@example.com".to_string()),
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: closed_loopback_backend_base_url(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: BTreeMap::new(),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let observed_revision = runtime_probe_refresh_revision();
    schedule_runtime_probe_refresh(&shared, "main", &main_home);

    assert!(
        wait_for_runtime_probe_refresh_since(
            ci_timing_upper_bound_ms(250, 1_000),
            observed_revision
        ),
        "loopback upstreams should still run through the background refresh path in tests"
    );
    wait_for_runtime_background_queues_idle();

    let runtime = shared.runtime.lock().expect("runtime lock should succeed");
    assert!(
        runtime.profile_probe_cache.contains_key("main"),
        "loopback probe refresh should still apply a probe result in tests"
    );
    drop(runtime);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        !log.contains(
            "profile_probe_refresh_suppressed profile=main reason=test_nonlocal_upstream"
        ),
        "loopback probe refresh should not be suppressed in tests"
    );
}

#[test]
fn runtime_proxy_responses_inflight_relief_times_out_without_relief() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(20, 20);

    let temp_dir = TestDir::new();
    let profile_home = temp_dir.path.join("homes/main");
    write_auth_json(&profile_home.join("auth.json"), "main-account");

    let usage = usage_with_main_windows(90, 3600, 90, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
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
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "main".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([(
                "main".to_string(),
                RuntimeProfileProbeCacheEntry {
                    checked_at: Local::now().timestamp(),
                    auth: AuthSummary {
                        label: "chatgpt".to_string(),
                        quota_compatible: true,
                    },
                    result: Ok(usage.clone()),
                },
            )]),
            profile_usage_snapshots: BTreeMap::from([("main".to_string(), snapshot)]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let _inflight_guard = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("inflight guard should be acquired");
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::json!({
            "input": "request should time out without inflight relief"
        })
        .to_string()
        .into_bytes(),
    };
    let response = proxy_runtime_responses_request(44, &request, &shared)
        .expect("responses request should return a local timeout result");

    let RuntimeResponsesReply::Buffered(parts) = response else {
        panic!("expected buffered responses reply");
    };
    assert_eq!(
        parts.status, 503,
        "unexpected status without inflight relief"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&parts.body).expect("response body should parse");
    assert_eq!(
        body.get("error")
            .and_then(|error| error.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("service_unavailable")
    );

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_started route=responses"),
        "responses inflight timeout should log wait start"
    );
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "responses inflight timeout should log wait finish"
    );
    assert!(
        log.contains("useful=false"),
        "timeout without relief should log useful=false"
    );
    assert!(
        log.contains("wake_source=timeout"),
        "timeout without relief should log wake_source=timeout"
    );
}

#[test]
fn runtime_proxy_wait_scopes_to_session_owner_relief() {
    let _budget_guard = ci_runtime_proxy_admission_wait_budget_guard(40, 250);

    let temp_dir = TestDir::new();
    let main_home = temp_dir.path.join("homes/main");
    let second_home = temp_dir.path.join("homes/second");
    write_auth_json(&main_home.join("auth.json"), "main-account");
    write_auth_json(&second_home.join("auth.json"), "second-account");

    let usage = usage_with_main_windows(90, 3600, 90, 604_800);
    let snapshot = runtime_profile_usage_snapshot_from_usage(&usage);
    let shared = runtime_rotation_proxy_shared(
        &temp_dir,
        RuntimeRotationState {
            paths: AppPaths {
                root: temp_dir.path.join("prodex"),
                state_file: temp_dir.path.join("prodex/state.json"),
                managed_profiles_root: temp_dir.path.join("prodex/profiles"),
                shared_codex_root: temp_dir.path.join("shared"),
                legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
            },
            state: AppState {
                active_profile: Some("second".to_string()),
                profiles: BTreeMap::from([
                    (
                        "main".to_string(),
                        ProfileEntry {
                            codex_home: main_home,
                            managed: true,
                            email: Some("main@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                    (
                        "second".to_string(),
                        ProfileEntry {
                            codex_home: second_home,
                            managed: true,
                            email: Some("second@example.com".to_string()),
                            provider: ProfileProvider::Openai,
                        },
                    ),
                ]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::from([(
                    "sess-main".to_string(),
                    ResponseProfileBinding {
                        profile_name: "main".to_string(),
                        bound_at: Local::now().timestamp(),
                    },
                )]),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: "second".to_string(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::from([
                (
                    "main".to_string(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: Local::now().timestamp(),
                        auth: AuthSummary {
                            label: "chatgpt".to_string(),
                            quota_compatible: true,
                        },
                        result: Ok(usage.clone()),
                    },
                ),
                (
                    "second".to_string(),
                    RuntimeProfileProbeCacheEntry {
                        checked_at: Local::now().timestamp(),
                        auth: AuthSummary {
                            label: "chatgpt".to_string(),
                            quota_compatible: true,
                        },
                        result: Ok(usage.clone()),
                    },
                ),
            ]),
            profile_usage_snapshots: BTreeMap::from([
                ("main".to_string(), snapshot),
                (
                    "second".to_string(),
                    runtime_profile_usage_snapshot_from_usage(&usage),
                ),
            ]),
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        },
        usize::MAX,
    );

    let main_inflight = acquire_runtime_profile_inflight_guard(&shared, "main", "responses_http")
        .expect("main inflight guard should be acquired");
    let second_inflight =
        acquire_runtime_profile_inflight_guard(&shared, "second", "responses_http")
            .expect("second inflight guard should be acquired");
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        drop(second_inflight);
    });

    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/backend-api/codex/responses".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("session_id".to_string(), "sess-main".to_string()),
        ],
        body: serde_json::json!({
            "input": "request should only count owner relief as useful"
        })
        .to_string()
        .into_bytes(),
    };
    let excluded_profiles = BTreeSet::new();
    assert!(
        !runtime_proxy_maybe_wait_for_interactive_inflight_relief(RuntimeInflightReliefWait::new(
            45,
            &request,
            &shared,
            &excluded_profiles,
            RuntimeRouteKind::Responses,
            Instant::now(),
            true,
            Some("main"),
        ),)
        .expect("owner-scoped wait should complete"),
        "non-owner release should not count as useful relief"
    );

    release
        .join()
        .expect("non-owner release thread should join");
    drop(main_inflight);

    let log = fs::read_to_string(&shared.log_path).expect("runtime log should be readable");
    assert!(
        log.contains("inflight_wait_finished route=responses"),
        "owner-scoped wait should log completion"
    );
    assert!(
        log.contains("useful=false"),
        "non-owner release should not be logged as useful relief"
    );
    assert!(
        log.contains("wake_source=inflight_release"),
        "non-owner release should still be logged as an inflight release wake"
    );
}
