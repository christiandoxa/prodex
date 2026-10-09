use super::*;

#[test]
fn parses_supported_custom_provider_capabilities() {
    let root = temp_dir("custom-provider-capabilities");
    fs::create_dir_all(&root).unwrap();
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        r#"
model_provider = "openai-custom"
[model_providers.openai-custom]
name = "OpenAI Custom"
base_url = "https://example.com/v1"
wire_api = "responses"
[model_providers.openai-custom.capabilities]
external_web_access = false
remote_compaction = "unsupported"
"#,
    )
    .unwrap();

    assert_eq!(
        codex_config_file_toml_value(
            &config_path,
            "model_providers.openai-custom.capabilities.external_web_access",
        )
        .unwrap(),
        Some(toml::Value::Boolean(false))
    );
    assert_eq!(
        codex_config_file_toml_value(
            &config_path,
            "model_providers.openai-custom.capabilities.remote_compaction",
        )
        .unwrap(),
        Some(toml::Value::String("unsupported".to_string()))
    );
}

#[test]
fn cli_override_reads_supported_custom_provider_capabilities_in_each_config_shape() {
    let args = [
        OsString::from("-cmodel_providers.openai-custom.capabilities.external_web_access=false"),
        OsString::from(
            "--config=model_providers.openai-custom.capabilities.remote_compaction=\"v2\"",
        ),
    ];

    assert_eq!(
        codex_cli_config_override_value(
            &args,
            "model_providers.openai-custom.capabilities.external_web_access",
        )
        .as_deref(),
        Some("false")
    );
    assert_eq!(
        codex_cli_config_override_value(
            &args,
            "model_providers.openai-custom.capabilities.remote_compaction",
        )
        .as_deref(),
        Some("v2")
    );
}
