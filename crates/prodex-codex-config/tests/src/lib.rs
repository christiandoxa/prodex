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
fn parses_exact_key_without_stopping_on_prefix_match() {
    let contents = r#"
            model_provider = "prodex-local"
            model = "qwen3-coder"
        "#;

    assert_eq!(
        parse_toml_string_assignment(contents, "model").as_deref(),
        Some("qwen3-coder")
    );
}

#[test]
fn parses_nested_toml_table_string_value() {
    let root = temp_dir("nested-table-string-value");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("config.toml"),
        "[model_providers.prodex-local]\nbase_url = 'http://127.0.0.1:8131/v1'\n",
    )
    .unwrap();

    assert_eq!(
        codex_config_value(&root, "model_providers.prodex-local.base_url").as_deref(),
        Some("http://127.0.0.1:8131/v1")
    );
}

#[test]
fn exact_config_value_preserves_empty_and_whitespace_strings() {
    let root = temp_dir("exact-config-values");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("config.toml"),
        "empty = ''\nspaced = ' 128000 '\n",
    )
    .unwrap();

    assert_eq!(
        codex_config_exact_value(&root, "empty").as_deref(),
        Some("")
    );
    assert_eq!(
        codex_config_exact_value(&root, "spaced").as_deref(),
        Some(" 128000 ")
    );
    assert_eq!(codex_config_value(&root, "empty"), None);
}

#[test]
fn exact_cli_override_preserves_empty_and_quoted_whitespace() {
    assert_eq!(
        codex_cli_config_override_exact_value(
            &[OsString::from("--config=model_context_window=''")],
            "model_context_window",
        )
        .as_deref(),
        Some("")
    );
    assert_eq!(
        codex_cli_config_override_exact_value(
            &[
                OsString::from("-c"),
                OsString::from("model_context_window=' 128000 '"),
            ],
            "model_context_window",
        )
        .as_deref(),
        Some(" 128000 ")
    );
    assert_eq!(
        codex_cli_config_override_value(
            &[OsString::from("--config=model_context_window=''")],
            "model_context_window",
        ),
        None
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
fn parses_valid_profile_v2_from_codex_args() {
    assert_eq!(
        codex_cli_profile_v2_name(&[
            OsString::from("exec"),
            OsString::from("--profile"),
            OsString::from("local_1-prod"),
        ])
        .as_deref(),
        Some("local_1-prod")
    );
    assert_eq!(
        codex_cli_profile_v2_name(&[OsString::from("--profile=team_2")]).as_deref(),
        Some("team_2")
    );
    assert_eq!(
        codex_cli_profile_v2_name(&[OsString::from("--profile-v2=team_2")]).as_deref(),
        Some("team_2")
    );
}

#[test]
fn ignores_path_like_profile_v2_names_for_config_lookup() {
    let root = temp_dir("profile-v2-path-like");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    fs::write(root.join("evil.config.toml"), "model_provider = 'openai'\n").unwrap();

    assert_eq!(
        codex_cli_profile_v2_name(&[OsString::from("--profile-v2=../evil")]),
        None
    );
    assert_eq!(
        codex_configured_model_provider_with_profile_v2(&root, Some("../evil")).as_deref(),
        Some("amazon-bedrock")
    );
}

#[test]
fn profile_v2_config_overlays_base_config_for_model_provider() {
    let root = temp_dir("profile-v2-overlay");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();
    fs::write(
        root.join("local.config.toml"),
        "model_provider = 'openai'\n",
    )
    .unwrap();

    assert_eq!(
        codex_configured_model_provider_with_profile_v2(&root, Some("local")).as_deref(),
        Some("openai")
    );
    assert!(codex_non_openai_model_provider_with_profile_v2(&root, None, Some("local")).is_none());
}

#[test]
fn profile_v2_config_is_used_for_args_when_no_cli_override_exists() {
    let root = temp_dir("profile-v2-args");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("config.toml"), "model_provider = 'openai'\n").unwrap();
    fs::write(
        root.join("bedrock.config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();

    let provider = codex_non_openai_model_provider_for_args(
        &root,
        &[OsString::from("exec"), OsString::from("--profile=bedrock")],
    )
    .unwrap();

    assert_eq!(provider.provider_id, "amazon-bedrock");
    assert_eq!(
        provider.source,
        CodexModelProviderSource::ProfileV2ConfigFile
    );
}

#[test]
fn cli_override_takes_precedence_over_profile_v2_config() {
    let root = temp_dir("profile-v2-cli-override");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("bedrock.config.toml"),
        "model_provider = 'amazon-bedrock'\n",
    )
    .unwrap();

    assert!(
        codex_non_openai_model_provider_for_args(
            &root,
            &[
                OsString::from("--profile-v2=bedrock"),
                OsString::from("--config=model_provider=openai"),
            ],
        )
        .is_none()
    );
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
