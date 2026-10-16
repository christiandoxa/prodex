#![cfg(all(feature = "mojo-runtime", prodex_mojo_required))]

use prodex_mojo_core::runtime_request_target::is_valid;

#[test]
fn runtime_request_target_caller_preserves_valid_queries_and_encoded_segments() {
    for value in [
        "/v1/responses",
        "/v1/items?query=./..",
        "/v1/%41",
        &format!("/{}", "x".repeat(8 * 1024 - 1)),
    ] {
        assert!(is_valid(value).unwrap(), "value={value:?}");
    }
}

#[test]
fn runtime_request_target_caller_rejects_ambiguous_or_malformed_targets() {
    for value in [
        "",
        "responses",
        "//v1/responses",
        "/v1//responses",
        "/v1/./responses",
        "/v1/../responses",
        "/v1/%2fresponses",
        "/v1/%2E/responses",
        "/v1/%25responses",
        "/v1/%zz",
        "/v1/%",
        "/v1/responses#fragment",
        "/v1/responses\n",
        "\u{00e9}",
        &format!("/{}", "x".repeat(8 * 1024)),
    ] {
        assert!(!is_valid(value).unwrap(), "value={value:?}");
    }
}
