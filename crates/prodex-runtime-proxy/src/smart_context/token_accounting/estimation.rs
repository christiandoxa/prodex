pub const SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN: u64 = 4;

pub fn smart_context_estimate_tokens_from_body_bytes(body_bytes: usize) -> u64 {
    #[cfg(feature = "mojo")]
    {
        crate::quota::mojo::smart_context_estimate_tokens_from_body_bytes(
            u64::try_from(body_bytes).unwrap_or(u64::MAX),
        )
    }

    #[cfg(not(feature = "mojo"))]
    {
        smart_context_estimate_tokens_from_body_bytes_rust(body_bytes)
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn smart_context_estimate_tokens_from_body_bytes_rust(body_bytes: usize) -> u64 {
    let body_bytes = u64::try_from(body_bytes).unwrap_or(u64::MAX);
    body_bytes.saturating_add(SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN - 1)
        / SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN
}

pub fn smart_context_estimate_tokens_from_body(body: &[u8]) -> u64 {
    #[cfg(feature = "mojo")]
    {
        crate::quota::mojo::smart_context_estimate_tokens_from_body(body)
            .expect("Mojo text token estimator rejected a Rust byte slice")
    }

    #[cfg(not(feature = "mojo"))]
    {
        smart_context_estimate_tokens_from_body_rust(body)
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn smart_context_estimate_tokens_from_body_rust(body: &[u8]) -> u64 {
    let byte_estimate = smart_context_estimate_tokens_from_body_bytes_rust(body.len());
    let Ok(text) = std::str::from_utf8(body) else {
        return byte_estimate;
    };
    smart_context_estimate_tokens_from_text(text).max(byte_estimate.saturating_div(2))
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_estimate_tokens_from_text(text: &str) -> u64 {
    let mut tokens = 0u64;
    let mut run = String::new();
    let mut run_kind = SmartContextEstimatorRunKind::Other;
    let mut structural = 0u64;
    let mut separators = 0u64;

    for ch in text.chars() {
        let kind = smart_context_estimator_run_kind(ch);
        if matches!(
            kind,
            SmartContextEstimatorRunKind::Word | SmartContextEstimatorRunKind::Number
        ) {
            if kind != run_kind {
                tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
                run.clear();
                run_kind = kind;
            }
            run.push(ch);
            continue;
        }

        tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
        run.clear();
        run_kind = SmartContextEstimatorRunKind::Other;

        if ch.is_whitespace() {
            if ch == '\n' {
                separators = separators.saturating_add(1);
            }
        } else if matches!(
            ch,
            '{' | '}' | '[' | ']' | ':' | ',' | '"' | '\'' | '`' | '(' | ')' | '<' | '>'
        ) {
            structural = structural.saturating_add(1);
        } else {
            tokens = tokens.saturating_add(1);
        }
    }

    tokens = tokens.saturating_add(smart_context_estimate_run_tokens(&run, run_kind));
    tokens
        .saturating_add(structural.saturating_add(3) / 4)
        .saturating_add(separators.saturating_add(7) / 8)
}

#[cfg(any(not(feature = "mojo"), test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::smart_context) enum SmartContextEstimatorRunKind {
    Word,
    Number,
    Other,
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_estimator_run_kind(
    ch: char,
) -> SmartContextEstimatorRunKind {
    if ch.is_ascii_alphabetic() || ch == '_' || ch == '-' {
        SmartContextEstimatorRunKind::Word
    } else if ch.is_ascii_digit() {
        SmartContextEstimatorRunKind::Number
    } else {
        SmartContextEstimatorRunKind::Other
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_estimate_run_tokens(
    run: &str,
    kind: SmartContextEstimatorRunKind,
) -> u64 {
    if run.is_empty() {
        return 0;
    }
    let chars = u64::try_from(run.chars().count()).unwrap_or(u64::MAX);
    match kind {
        SmartContextEstimatorRunKind::Word => chars.saturating_add(3) / 4,
        SmartContextEstimatorRunKind::Number => chars.saturating_add(2) / 3,
        SmartContextEstimatorRunKind::Other => chars,
    }
    .max(1)
}

#[cfg(all(test, feature = "mojo"))]
mod tests {
    use super::{
        smart_context_estimate_tokens_from_body, smart_context_estimate_tokens_from_body_rust,
    };

    #[test]
    fn mojo_body_token_estimator_matches_seeded_rust_oracle() {
        const FRAGMENTS: [&str; 27] = [
            "a", "Z", "0", "9", "_", "-", " ", "\n", "\r", "\u{1c}", "\u{2003}", "{", "}", "[",
            "]", ":", ",", "\"", "'", "`", "(", ")", "<", ">", "é", "界", "🙂",
        ];
        let mut seed = 0x4d59_5df4_d0f3_3173_u64;
        for case in 0..128 {
            let mut body = String::new();
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let fragments = (seed >> 58) as usize;
            for _ in 0..fragments {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                body.push_str(FRAGMENTS[(seed as usize) % FRAGMENTS.len()]);
            }
            let bytes = body.as_bytes();
            let mojo = prodex_mojo_core::runtime::smart_context_estimate_tokens_from_body(bytes)
                .expect("valid Rust byte slice is accepted by Mojo");
            assert_eq!(
                mojo,
                smart_context_estimate_tokens_from_body_rust(bytes),
                "case={case}, body={body:?}"
            );
            assert_eq!(
                smart_context_estimate_tokens_from_body(bytes),
                mojo,
                "case={case}, body={body:?}"
            );
        }

        let invalid_utf8 = [0xff, b'a', 0xf0, 0x80];
        let mojo =
            prodex_mojo_core::runtime::smart_context_estimate_tokens_from_body(&invalid_utf8)
                .expect("invalid UTF-8 uses the byte estimate");
        assert_eq!(
            mojo,
            smart_context_estimate_tokens_from_body_rust(&invalid_utf8)
        );
        assert_eq!(smart_context_estimate_tokens_from_body(&invalid_utf8), mojo);
    }
}
