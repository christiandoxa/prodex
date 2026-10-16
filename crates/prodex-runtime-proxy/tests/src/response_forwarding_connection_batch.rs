use super::*;

#[test]
fn compiled_mojo_filters_multiple_connection_headers_without_leaking_hop_bytes() {
    let headers = runtime_forward_binary_response_headers([
        ("Connection", b"Keep-Alive, X-Hop-One".as_slice()),
        ("Connection", b" X-HOP-TWO , trailer".as_slice()),
        ("X-Hop-One", b"discard-1".as_slice()),
        ("x-hop-two", b"discard-2".as_slice()),
        ("Trailer", b"discard-trailer".as_slice()),
        ("Date", b"2026-10-16".as_slice()),
        ("x-codex-turn-state", b"opaque-turn-state".as_slice()),
        ("x-opaque-binary", b"\xff\x80".as_slice()),
    ]);
    assert_eq!(
        headers,
        [
            ("Date".to_owned(), b"2026-10-16".to_vec()),
            (
                "x-codex-turn-state".to_owned(),
                b"opaque-turn-state".to_vec()
            ),
            ("x-opaque-binary".to_owned(), b"\xff\x80".to_vec()),
        ],
    );
    let names = runtime_forward_text_response_headers([
        ("connection", " X-One "),
        ("CONNECTION", " X-Two "),
        ("X-One", "drop-1"),
        ("x-two", "drop-2"),
        ("x-real", "keep"),
    ]);
    assert_eq!(names, [("x-real".to_owned(), "keep".to_owned())]);
}
