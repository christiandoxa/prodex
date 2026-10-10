use super::*;

#[test]
fn model_registry_resolves_known_models_after_normalization() {
    let window = smart_context_model_context_window(Some("\u{2003}GPT-5.1-Codex\u{3000}"))
        .expect("known model should resolve");

    assert_eq!(window.tokens, 200_000);
    assert_eq!(window.source, "model_registry");
    assert_eq!(
        window.registry_version,
        SMART_CONTEXT_MODEL_REGISTRY_VERSION
    );
}

#[test]
fn model_registry_normalization_rejects_controls_and_keeps_exact_bound() {
    let exact = "x".repeat(128);
    assert_eq!(
        smart_context_normalized_model_name(Some(&exact)).as_deref(),
        Some(exact.as_str())
    );
    assert_eq!(
        smart_context_normalized_model_name(Some(&"x".repeat(129))),
        None
    );
    assert_eq!(
        smart_context_normalized_model_name(Some("gpt-5\u{0007}")),
        None
    );
}

#[test]
fn model_registry_resolves_catalogued_openai_windows() {
    for model in [
        "gpt-5.6-luna",
        "gpt-6-astra",
        "gpt-6.1-sol",
        "gpt-6-sol",
        "gpt-6-luna",
    ] {
        assert_eq!(
            smart_context_model_context_window(Some(model)).map(|window| window.tokens),
            Some(872_000),
            "{model}"
        );
    }
    assert_eq!(
        smart_context_model_context_window(Some("local-gpt-5.6-luna")),
        None
    );
}

#[test]
fn model_registry_resolves_known_provider_windows() {
    assert_eq!(
        smart_context_model_context_window(Some("gemini-3-pro")).map(|window| window.tokens),
        Some(1_048_576)
    );
    assert_eq!(
        smart_context_model_context_window(Some("claude-opus-4-6")).map(|window| window.tokens),
        Some(200_000)
    );
    assert_eq!(
        smart_context_model_context_window(Some("deepseek-chat")).map(|window| window.tokens),
        Some(128_000)
    );
    assert_eq!(
        smart_context_model_context_window(Some("deepseek-custom")).map(|window| window.tokens),
        Some(128_000)
    );
}

#[test]
fn model_registry_leaves_unknown_or_invalid_models_unresolved() {
    for model in ["gpt-5.3-codex-spark", "spark", "local-test"] {
        assert_eq!(
            smart_context_model_context_window(Some(model)),
            None,
            "retired or unknown model must not get a context window"
        );
    }
    assert_eq!(
        smart_context_model_context_window(Some("bad\u{0007}model")),
        None
    );
    assert_eq!(smart_context_model_context_window(None), None);
}
