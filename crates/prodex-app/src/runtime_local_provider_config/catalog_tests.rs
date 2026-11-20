#[test]
fn local_catalog_preserves_original_complete_file_bytes() {
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};

    let home = temp_codex_home("schema-capture");
    let path = home.join(LOCAL_MODEL_CATALOG_FILE);
    let long_model = "x".repeat(4097);
    let cases = [
        ("ordinary", "synthetic-model", 128_000, 120_000),
        ("raw-unicode", " \tcustom\"\\\n雪 ", 0, u64::MAX),
        ("empty-slug", "", 1, 0),
        ("zero-budget", "zero-model", 0, 0),
        ("max-unsigned", "max-model", u64::MAX, u64::MAX),
        ("compact-above-context", "compact-model", 8, u64::MAX),
        ("long-slug", long_model.as_str(), 1, u64::MAX),
    ];
    let mut summary = Vec::new();
    for (name, model, context, compact) in cases {
        write_local_model_catalog(&path, model, context, compact).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        let value: Value = serde_json::from_str(&contents).unwrap();
        let digest = Sha256::digest(contents.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        summary.push(json!({"name":name,"bytes":contents.len(),"sha256":digest,
            "context_window":value["models"][0]["context_window"],
            "auto_compact_token_limit":value["models"][0]["auto_compact_token_limit"]}));
    }
    fs::remove_dir_all(&home).unwrap();
    let expected: Value = serde_json::from_str(include_str!("catalog_baseline.json")).unwrap();
    assert_eq!(Value::Array(summary), expected);
}

#[test]
fn local_long_context_timeout_defaults_and_preserves_codex_overrides() {
    let home = temp_codex_home("long-context-timeout");
    fs::create_dir_all(&home).unwrap();
    let key = "model_providers.prodex-local.stream_idle_timeout_ms";
    let mut args = [
        "-c",
        "model_provider=\"prodex-local\"",
        "-c",
        "model_context_window=212992",
        "exec",
        "-",
    ]
    .map(OsString::from)
    .to_vec();
    let projected = preview_local_provider_catalog_codex_args(&home, &args).unwrap();
    assert_eq!(
        codex_cli_config_override_value(&projected, key).as_deref(),
        Some("900000")
    );

    let config = home.join("config.toml");
    fs::write(
        &config,
        "[model_providers.prodex-local]\nstream_idle_timeout_ms = 300000\n",
    )
    .unwrap();
    let projected = preview_local_provider_catalog_codex_args(&home, &args).unwrap();
    assert!(codex_cli_config_override_value(&projected, key).is_none());
    fs::remove_file(config).unwrap();

    args.splice(0..0, ["-c".into(), format!("{key}=45000").into()]);
    let projected = preview_local_provider_catalog_codex_args(&home, &args).unwrap();
    assert_eq!(
        codex_cli_config_override_value(&projected, key).as_deref(),
        Some("45000")
    );
    args.drain(0..2);
    args[3] = "model_context_window=131071".into();
    let projected = preview_local_provider_catalog_codex_args(&home, &args).unwrap();
    assert!(codex_cli_config_override_value(&projected, key).is_none());
    fs::remove_dir_all(home).unwrap();
}
