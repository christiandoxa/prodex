use super::*;

#[test]
fn runtime_proxy_passthrough_keeps_supported_custom_provider_configuration() {
    let input = vec![
        OsString::from("exec"),
        OsString::from("-c"),
        OsString::from("model_provider=\"openai-custom\""),
        OsString::from("--config=model_providers.openai-custom.name=\"OpenAI Custom\""),
        OsString::from("-cmodel_providers.openai-custom.base_url=\"https://example.com/v1\""),
        OsString::from("-c"),
        OsString::from("model_providers.openai-custom.wire_api=\"responses\""),
        OsString::from(
            "--config=model_providers.openai-custom.capabilities.external_web_access=false",
        ),
        OsString::from("-cmodel_providers.openai-custom.capabilities.remote_compaction=\"v2\""),
        OsString::from("hello"),
    ];

    assert_eq!(runtime_proxy_codex_passthrough_args(None, &input), input);
}

#[test]
fn runtime_proxy_codex_args_keep_explicit_governed_capabilities_and_resume_identity() {
    let args = runtime_proxy_codex_args(
        "127.0.0.1:4455".parse().expect("socket addr"),
        &[
            OsString::from("exec"),
            OsString::from("resume"),
            OsString::from("00000000-0000-4000-8000-000000000001"),
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-openai-governed-http\""),
            OsString::from(
                "-cmodel_providers.prodex-openai-governed-http.capabilities.external_web_access=false",
            ),
            OsString::from(
                "--config=model_providers.prodex-openai-governed-http.capabilities.remote_compaction=\"unsupported\"",
            ),
        ],
    );

    assert_eq!(
        codex_resume_session_id(&args),
        Some("00000000-0000-4000-8000-000000000001")
    );
    assert!(args.iter().any(|arg| {
        arg == "-cmodel_providers.prodex-openai-governed-http.capabilities.external_web_access=false"
    }));
    assert!(args.iter().any(|arg| {
        arg == "--config=model_providers.prodex-openai-governed-http.capabilities.remote_compaction=\"unsupported\""
    }));
}

#[test]
fn runtime_proxy_passthrough_keeps_literal_custom_provider_config_after_separator() {
    let input = vec![
        OsString::from("exec"),
        OsString::from("--"),
        OsString::from(
            "--config=model_providers.openai-custom.capabilities.external_web_access=false",
        ),
        OsString::from("literal tail"),
    ];

    assert_eq!(runtime_proxy_codex_passthrough_args(None, &input), input);
}
