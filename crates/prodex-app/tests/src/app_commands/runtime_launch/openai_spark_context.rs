use super::*;

#[test]
fn runtime_launch_injects_openai_spark_context_defaults() {
    let root = temp_dir("openai-spark-context-defaults");
    fs::create_dir_all(&root).unwrap();

    let args = runtime_launch_openai_spark_context_codex_args(
        &root,
        &[
            OsString::from("--model"),
            OsString::from("gpt-5.3-codex-spark"),
        ],
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(rendered.contains(&"model_context_window=128000".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=115200".to_string()));
    assert_eq!(
        &rendered[rendered.len() - 2..],
        ["--model", "gpt-5.3-codex-spark"]
    );
}

#[test]
fn runtime_launch_uses_cached_openai_spark_context_metadata() {
    let root = temp_dir("openai-spark-context-cache");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("models_cache.json"),
        r#"{"models":[{"slug":"gpt-5.3-codex-spark","context_window":128000}]}"#,
    )
    .unwrap();

    let args = runtime_launch_openai_spark_context_codex_args(
        &root,
        &[
            OsString::from("--model"),
            OsString::from("gpt-5.3-codex-spark"),
        ],
    );

    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(rendered.contains(&"model_context_window=128000".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=115200".to_string()));
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

    let args = runtime_launch_openai_spark_context_codex_args(&root, &[]);
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(!rendered.contains(&"model_context_window=272000".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=244800".to_string()));
}

#[test]
fn runtime_launch_does_not_inject_openai_spark_defaults_for_copilot_provider() {
    let root = temp_dir("copilot-spark-context-defaults");
    fs::create_dir_all(&root).unwrap();

    let args = runtime_launch_openai_spark_context_codex_args(
        &root,
        &[
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-copilot\""),
            OsString::from("--model"),
            OsString::from("gpt-5.3-codex-spark"),
        ],
    );

    assert!(!args.iter().any(|arg| arg == "model_context_window=128000"));
    assert!(
        !args
            .iter()
            .any(|arg| arg == "model_auto_compact_token_limit=115200")
    );
}
