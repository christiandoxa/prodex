pub const SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN: u64 = 4;

pub fn smart_context_estimate_tokens_from_body_bytes(body_bytes: usize) -> u64 {
    let body_bytes = u64::try_from(body_bytes).unwrap_or(u64::MAX);
    body_bytes.saturating_add(SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN - 1)
        / SMART_CONTEXT_ESTIMATED_BYTES_PER_TOKEN
}

pub fn smart_context_estimate_tokens_from_body(body: &[u8]) -> u64 {
    let Ok(text) = std::str::from_utf8(body) else {
        return smart_context_estimate_tokens_from_body_bytes(body.len());
    };
    smart_context_estimate_tokens_from_text(text)
        .max(smart_context_estimate_tokens_from_body_bytes(body.len()).saturating_div(2))
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::smart_context) enum SmartContextEstimatorRunKind {
    Word,
    Number,
    Other,
}

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
