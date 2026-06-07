use super::*;

#[test]
fn super_external_providers_enable_live_web_search() {
    for provider in ["anthropic", "copilot", "deepseek", "gemini"] {
        let args = parse_super_as_caveman(&[
            "prodex",
            "s",
            "--provider",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "review",
        ]);
        let rendered = args
            .codex_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(
            rendered.contains(&"web_search=\"live\"".to_string()),
            "{provider} should enable live web search"
        );
    }
}

#[test]
fn super_provider_short_aliases_expand_to_provider_flag() {
    for provider in ["deepseek", "gemini"] {
        let alias_args = parse_super_as_caveman(&[
            "prodex",
            "s",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "review",
        ]);
        let explicit_args = parse_super_as_caveman(&[
            "prodex",
            "s",
            "--provider",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "review",
        ]);

        assert_same_caveman_args(alias_args, explicit_args);
    }
}

#[test]
fn super_command_provider_short_aliases_expand_to_provider_flag() {
    let args = parse_super_as_caveman(&[
        "prodex",
        "super",
        "gemini",
        "--api-key",
        "test-key",
        "exec",
        "review",
    ]);

    assert_eq!(args.external_provider, Some(SuperExternalProvider::Gemini));
    assert_eq!(
        args.codex_args.last().and_then(|arg| arg.to_str()),
        Some("review")
    );
}

#[test]
fn super_gemini_provider_enables_native_image_generation_only_for_gemini() {
    for provider in ["anthropic", "copilot", "deepseek", "gemini"] {
        let args = parse_super_as_caveman(&[
            "prodex",
            "s",
            "--provider",
            provider,
            "--api-key",
            "test-key",
            "exec",
            "draw",
        ]);
        let rendered = args
            .codex_args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let expected = format!("features.image_generation={}", provider == "gemini");

        assert!(
            rendered.contains(&expected),
            "{provider} image generation feature mismatch"
        );
    }
}
