use super::*;

#[test]
fn model_catalog_matches_upstream_rust_0_123_0() {
    let ids = runtime_proxy_responses_model_descriptors()
        .iter()
        .map(|descriptor| descriptor.id)
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec!["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.2"]
    );
    assert!(!ids.contains(&"gpt-5.2-codex"));
    assert!(!ids.contains(&"gpt-5"));
}
