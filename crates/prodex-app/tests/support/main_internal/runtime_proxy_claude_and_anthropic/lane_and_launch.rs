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
        None,
    );
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
        None,
    );
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
    let temp_dir = TestDir::isolated();
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
fn ensure_runtime_proxy_claude_launch_config_seeds_onboarding_and_project_trust() {
    let temp_dir = TestDir::isolated();
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
    assert_eq!(additional_model_options.len(), 1);
    assert!(!additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.4")
    }));
    assert!(additional_model_options.iter().any(|entry| {
        entry.get("value").and_then(serde_json::Value::as_str) == Some("gpt-5.2")
            && entry.get("label").and_then(serde_json::Value::as_str) == Some("gpt-5.2")
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
    let temp_dir = TestDir::isolated();
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
    let temp_dir = TestDir::isolated();
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
    let temp_dir = TestDir::isolated();
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
    let temp_dir = TestDir::isolated();
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
