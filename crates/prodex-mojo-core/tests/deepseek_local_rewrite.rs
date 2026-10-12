use prodex_mojo_core::deepseek_local_rewrite::{
    DeepSeekLocalRewriteRoute, DeepSeekTranslationStatus, route,
};

#[test]
fn compiled_mojo_routes_native_translation_and_safe_auto_fallback() {
    let native = route(
        0,
        br#"{"web_search_options":{}}"#,
        DeepSeekTranslationStatus::Lossless,
        None,
    )
    .unwrap();
    assert_eq!(native, DeepSeekLocalRewriteRoute::NativeMessages);

    let fallback = route(
        0,
        br#"{"web_search_options":{}}"#,
        DeepSeekTranslationStatus::Rejected,
        Some("Anthropic Messages does not translate chat field `tools`"),
    )
    .unwrap();
    assert_eq!(fallback, DeepSeekLocalRewriteRoute::AutoChatFallback);
}

#[test]
fn compiled_mojo_preserves_chat_and_terminal_incompatibility_boundaries() {
    assert_eq!(
        route(
            1,
            br#"{"messages":[]}"#,
            DeepSeekTranslationStatus::Rejected,
            Some("translated message must be an object"),
        )
        .unwrap(),
        DeepSeekLocalRewriteRoute::Chat
    );
    assert_eq!(
        route(
            3,
            br#"{"web_search_options":{}}"#,
            DeepSeekTranslationStatus::Rejected,
            Some("Anthropic Messages does not translate chat field `tools`"),
        )
        .unwrap(),
        DeepSeekLocalRewriteRoute::Incompatible
    );
}

#[test]
fn compiled_mojo_rejects_malformed_and_oversized_inputs() {
    assert!(route(0, b"{", DeepSeekTranslationStatus::Lossless, None,).is_err());
    assert!(
        route(
            0,
            &vec![b'x'; 4 * 1024 * 1024 + 1],
            DeepSeekTranslationStatus::Lossless,
            None,
        )
        .is_err()
    );
}
