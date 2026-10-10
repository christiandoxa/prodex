#![cfg(all(feature = "mojo-runtime", feature = "mojo-rich", prodex_mojo_required))]

use prodex_mojo_core::rich::runtime_proxy_mount_path;

#[test]
fn compiled_mojo_preserves_mount_trim_and_slash_boundaries() {
    for (input, expected) in [
        ("", ""),
        ("   ", ""),
        ("/", ""),
        (" /// ", "/"),
        ("  /v1///  ", "/v1"),
        ("\u{2003}//api//v1//\u{3000}", "/api//v1"),
        ("proxy", "/proxy"),
    ] {
        assert_eq!(
            runtime_proxy_mount_path(input).unwrap(),
            expected,
            "input={input:?}"
        );
    }
}
