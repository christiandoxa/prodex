use super::*;

#[test]
fn runtime_launch_keeps_upstream_context_defaults_without_cache() {
    let root = temp_dir("openai-context-passthrough-no-cache");
    fs::create_dir_all(&root).unwrap();
    for model in ["gpt-5.3-codex", "gpt-6.1-sol", "gpt-6-luna", "future-model"] {
        let original = [OsString::from("--model"), OsString::from(model)];
        let result = runtime_launch_openai_model_context_codex_args(&root, &original).unwrap();
        assert_eq!(
            result, original,
            "must not infer context defaults for {model}"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_launch_does_not_promote_cached_maximum_or_invent_compaction_threshold() {
    let root = temp_dir("openai-context-passthrough-cache");
    fs::create_dir_all(&root).unwrap();
    let cache = r#"{"models":[{"slug":"gpt-6.1-sol","context_window":272000,"max_context_window":872000,"effective_context_window_percent":95}]}"#;
    let config = "model = \"gpt-6.1-sol\"\n";
    fs::write(root.join("models_cache.json"), cache).unwrap();
    fs::write(root.join("config.toml"), config).unwrap();
    for original in [vec![], vec!["--model".into(), "gpt-6.1-sol".into()]] {
        assert_eq!(
            runtime_launch_openai_model_context_codex_args(&root, &original).unwrap(),
            original
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("models_cache.json")).unwrap(),
        cache
    );
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).unwrap(),
        config
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_launch_preserves_explicit_context_and_compaction_overrides_exactly() {
    let root = temp_dir("openai-context-passthrough-explicit");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "model_context_window = 600000\n").unwrap();
    let original = [
        "--model",
        "gpt-6.1-sol",
        "-c",
        "model_context_window=500000",
        "--config=model_auto_compact_token_limit=430000",
        "-c",
        r#"model_auto_compact_token_limit_scope="body_after_prefix""#,
        "-c",
        "model_context_window=700000",
        "exec",
        "resume",
        "00000000-0000-7000-8000-000000000042",
        "continue",
    ]
    .map(OsString::from);
    assert_eq!(
        runtime_launch_openai_model_context_codex_args(&root, &original).unwrap(),
        original
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_launch_defers_profile_values_and_invalid_context_to_codex() {
    let root = temp_dir("openai-context-passthrough-profile");
    fs::create_dir_all(&root).unwrap();
    let profile = "model = \"gpt-6.1-sol\"\nmodel_context_window = 640000\nmodel_auto_compact_token_limit = 520000\n";
    fs::write(root.join("deep.config.toml"), profile).unwrap();
    for original in [
        vec!["-p".into(), "deep".into()],
        vec![
            "-c".into(),
            "model_context_window=invalid".into(),
            "--model".into(),
            "gpt-6.1-sol".into(),
        ],
        vec!["-c".into(), "model_auto_compact_token_limit=0".into()],
    ] {
        assert_eq!(
            runtime_launch_openai_model_context_codex_args(&root, &original).unwrap(),
            original
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("deep.config.toml")).unwrap(),
        profile
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn runtime_launch_does_not_inject_openai_defaults_into_other_providers() {
    let root = temp_dir("openai-context-passthrough-provider");
    let original = [
        "-c",
        r#"model_provider="prodex-copilot""#,
        "--model",
        "gpt-6-luna",
    ]
    .map(OsString::from);
    assert_eq!(
        runtime_launch_openai_model_context_codex_args(&root, &original).unwrap(),
        original
    );
}
