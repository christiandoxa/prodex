#[test]
fn offrail_catalog_uses_supported_reasoning_and_eager_tools() {
    let home = temp_codex_home("offrail-capabilities");
    let args = ["-c", "model_provider=\"prodex-local\"", "-c", "model=\"offrail-27b\"", "-c", "model_context_window=118272", "-c", "model_auto_compact_token_limit=100000"]
        .map(OsString::from);
    fs::create_dir_all(&home).unwrap();
    fs::write(home.join("config.toml"), "model=\"gpt-5.6-luna\"\nmodel_reasoning_effort=\"max\"\nmodel_context_window=\"16000\"\nmodel_auto_compact_token_limit=\"14000\"\n").unwrap();
    let result = prepare_local_provider_catalog_codex_args(&home, &args).unwrap();
    let path = home.join(LOCAL_MODEL_CATALOG_FILE);
    let catalog: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(catalog_efforts(&path), ["low", "medium", "xhigh"]);
    assert_eq!(catalog["models"][0]["default_reasoning_level"], "medium");
    assert_eq!(catalog["models"][0]["supports_search_tool"], false);
    assert_eq!(catalog["models"][0]["context_window"], 118272);
    assert_eq!(catalog["models"][0]["auto_compact_token_limit"], 100000);
    assert_eq!(codex_cli_config_override_value(&result, "model_reasoning_effort").as_deref(), Some("medium"));
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn offrail_reasoning_honors_explicit_supported_and_rejects_invalid_before_launch() {
    for effort in ["low", "medium", "xhigh", "high", "max"] {
        let home = temp_codex_home("offrail-explicit-effort");
        let args = vec!["-c".into(), "model_provider=\"prodex-local\"".into(),
            "-c".into(), "model=\"offrail-27b\"".into(), "-c".into(),
            format!("model_reasoning_effort=\"{effort}\"").into()];
        let result = prepare_local_provider_catalog_codex_args(&home, &args);
        if matches!(effort, "high" | "max") {
            assert!(result.unwrap_err().to_string().contains("unsupported"));
        } else {
            assert_eq!(codex_cli_config_override_value(&result.unwrap(), "model_reasoning_effort").as_deref(), Some(effort));
            fs::remove_dir_all(home).unwrap();
        }
    }
}

#[test]
fn local_chat_model_policy_is_exact_and_cloud_catalog_remains_unchanged() {
    for (model, expected) in [("offrail-27b", true), ("bonsai-2-27b", true),
        ("local/qwen", false), ("gpt-5.6-luna", false), ("offrail-27b-remote", false), ("", false)] {
        assert_eq!(prodex_mojo_core::json::local_model_uses_chat_completions(model).unwrap(), expected);
        let catalog = render_local_runtime_model_catalog(model, 16000, 12000).unwrap();
        assert_eq!(catalog["supports_search_tool"], !expected);
        assert_eq!(catalog["default_reasoning_level"], if expected { "medium" } else { "high" });
    }
}
