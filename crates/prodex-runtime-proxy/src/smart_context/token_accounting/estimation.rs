pub const SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN: u64 = 4;

pub fn smart_context_estimate_tokens_from_body_bytes(body_bytes: usize) -> u64 {
    prodex_mojo_core::runtime::smart_context_estimate_tokens_from_body_bytes(
        u64::try_from(body_bytes).unwrap_or(u64::MAX),
    )
}

pub fn smart_context_estimate_tokens_from_body(body: &[u8]) -> u64 {
    prodex_mojo_core::runtime::smart_context_estimate_tokens_from_body(body)
        .expect("Mojo text token estimator rejected a Rust byte slice")
}

#[cfg(test)]
mod tests {
    use super::smart_context_estimate_tokens_from_body;

    #[test]
    fn body_token_estimator_preserves_text_and_byte_floors() {
        for (body, expected) in [
            (&b""[..], 0),
            (&b"word "[..], 1),
            (&b"alpha 123"[..], 3),
            (&b"{}"[..], 1),
            (&b"\x1c"[..], 1),
            (&b"\n\n\n\n\n\n\n\n"[..], 1),
            (&[b' '; 100][..], 12),
            (&[0xff, b'a', 0xf0, 0x80][..], 1),
            ("é".as_bytes(), 1),
            ("界".as_bytes(), 1),
        ] {
            assert_eq!(
                smart_context_estimate_tokens_from_body(body),
                expected,
                "{body:?}"
            );
        }
    }
}
