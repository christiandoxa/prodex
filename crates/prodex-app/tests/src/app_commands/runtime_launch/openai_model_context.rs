use super::*;

#[test]
fn runtime_launch_uses_catalog_context_for_known_openai_model_without_cache() {
    let root = temp_dir("openai-model-context-catalog");
    fs::create_dir_all(&root).unwrap();

    let args = runtime_launch_openai_model_context_codex_args(
        &root,
        &[OsString::from("--model"), OsString::from("gpt-5.3-codex")],
    )
    .unwrap();
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(rendered.contains(&"model_context_window=400000".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=360000".to_string()));
    assert_eq!(
        &rendered[rendered.len() - 2..],
        ["--model", "gpt-5.3-codex"]
    );
}

#[test]
fn runtime_launch_leaves_unknown_openai_model_context_unset_without_metadata() {
    let root = temp_dir("openai-model-context-unknown");
    fs::create_dir_all(&root).unwrap();
    let original = [OsString::from("--model"), OsString::from("gpt-5.9-custom")];

    let args = runtime_launch_openai_model_context_codex_args(&root, &original).unwrap();

    assert_eq!(args, original);
}

#[test]
fn runtime_launch_uses_cached_openai_model_context_metadata() {
    let root = temp_dir("openai-model-context-cache");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("models_cache.json"),
        r#"{"models":[{"slug":"gpt-5.3-codex","context_window":400000}]}"#,
    )
    .unwrap();

    let args = runtime_launch_openai_model_context_codex_args(
        &root,
        &[OsString::from("--model"), OsString::from("gpt-5.3-codex")],
    )
    .unwrap();

    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(rendered.contains(&"model_context_window=400000".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=360000".to_string()));
}

#[test]
fn runtime_launch_injects_cached_openai_gpt5_context_metadata() {
    let root = temp_dir("openai-gpt5-context-cache");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), r#"model = "gpt-5.5""#).unwrap();
    fs::write(
        root.join("models_cache.json"),
        r#"{"models":[{"slug":"gpt-5.5","context_window":272000}]}"#,
    )
    .unwrap();

    let args = runtime_launch_openai_model_context_codex_args(&root, &[]).unwrap();
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(!rendered.contains(&"model_context_window=272000".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=244800".to_string()));
}

#[test]
fn runtime_launch_does_not_inject_openai_model_defaults_for_copilot_provider() {
    let root = temp_dir("copilot-model-context-defaults");
    fs::create_dir_all(&root).unwrap();

    let args = runtime_launch_openai_model_context_codex_args(
        &root,
        &[
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-copilot\""),
            OsString::from("--model"),
            OsString::from("gpt-5.3-codex"),
        ],
    )
    .unwrap();

    assert!(!args.iter().any(|arg| arg == "model_context_window=400000"));
    assert!(
        !args
            .iter()
            .any(|arg| arg == "model_auto_compact_token_limit=360000")
    );
}
