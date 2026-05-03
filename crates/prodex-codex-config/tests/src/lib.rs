use super::*;

#[test]
fn parses_model_provider_from_config_toml() {
    let contents = r#"
            model_provider = "amazon-bedrock"
            model = "gpt-5.4"
        "#;

    assert_eq!(
        parse_toml_string_assignment(contents, "model_provider").as_deref(),
        Some("amazon-bedrock")
    );
}

#[test]
fn cli_override_takes_precedence_over_config_file() {
    let root = temp_dir("cli-override-precedence");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "model_provider = 'openai'\n").unwrap();

    let provider = codex_non_openai_model_provider(
        &root,
        codex_cli_config_override_value(
            &[
                OsString::from("--config"),
                OsString::from("model_provider='amazon-bedrock'"),
            ],
            "model_provider",
        )
        .as_deref(),
    )
    .unwrap();

    assert_eq!(provider.provider_id, "amazon-bedrock");
    assert_eq!(provider.source, CodexModelProviderSource::CliOverride);
}

#[test]
fn explicit_openai_override_clears_non_openai_config() {
    let root = temp_dir("explicit-openai-override");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();

    let provider = codex_non_openai_model_provider(
        &root,
        codex_cli_config_override_value(
            &[OsString::from("--config=model_provider=openai")],
            "model_provider",
        )
        .as_deref(),
    );

    assert!(provider.is_none());
}

#[test]
fn fast_service_tier_config_does_not_parse_as_model_provider() {
    let root = temp_dir("fast-service-tier-not-model-provider");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("config.toml"),
        "service_tier = null\n[notice]\nfast_default_opt_out = true\n",
    )
    .unwrap();

    assert!(codex_configured_model_provider(&root).is_none());
    assert!(codex_non_openai_model_provider(&root, None).is_none());
    assert_eq!(
        codex_cli_config_override_value(
            &[
                OsString::from("-c"),
                OsString::from("service_tier=null"),
                OsString::from("--config=notice.fast_default_opt_out=true"),
            ],
            "model_provider",
        ),
        None
    );
}

#[test]
fn model_provider_override_survives_fast_service_tier_config() {
    let root = temp_dir("model-provider-with-fast-service-tier");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();

    let override_value = codex_cli_config_override_value(
        &[
            OsString::from("-c"),
            OsString::from("service_tier=null"),
            OsString::from("--config=notice.fast_default_opt_out=true"),
            OsString::from("--config"),
            OsString::from("model_provider=openai"),
        ],
        "model_provider",
    );

    assert_eq!(override_value.as_deref(), Some("openai"));
    assert!(codex_non_openai_model_provider(&root, override_value.as_deref()).is_none());
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "prodex-codex-config-{name}-{}-{}",
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
