use super::*;

#[test]
fn model_registry_resolves_known_models_after_normalization() {
    let window = smart_context_model_context_window(Some(" GPT-5.1-Codex "))
        .expect("known model should resolve");

    assert_eq!(window.tokens, 200_000);
    assert_eq!(window.source, "model_registry");
    assert_eq!(
        window.registry_version,
        SMART_CONTEXT_MODEL_REGISTRY_VERSION
    );
}

#[test]
fn model_registry_resolves_openai_codex_spark_window() {
    assert_eq!(
        smart_context_model_context_window(Some("gpt-5.3-codex-spark")).map(|window| window.tokens),
        Some(128_000)
    );
    assert_eq!(
        smart_context_model_context_window(Some("GPT-5.3-Spark")).map(|window| window.tokens),
        Some(128_000)
    );
}

#[test]
fn model_registry_uses_family_defaults_for_known_large_window_providers() {
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
        Some(1_048_576)
    );
}

#[test]
fn model_registry_leaves_unknown_or_invalid_models_unresolved() {
    assert_eq!(smart_context_model_context_window(Some("local-test")), None);
    assert_eq!(
        smart_context_model_context_window(Some("bad\u{0007}model")),
        None
    );
    assert_eq!(smart_context_model_context_window(None), None);
}
