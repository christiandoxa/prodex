use super::*;

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
