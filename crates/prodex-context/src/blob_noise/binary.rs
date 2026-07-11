use super::{ContextBlobNoiseFinding, ContextBlobNoiseKind};

pub(super) fn detect_binaryish_text_noise_supplement(
    input: &str,
) -> Option<ContextBlobNoiseFinding> {
    let total_chars = input.chars().count();
    if total_chars < 16 {
        return None;
    }

    let mut suspicious_chars = 0usize;
    let mut nul_chars = 0usize;
    let mut replacement_chars = 0usize;
    let mut first_line = None;
    for (line_index, line) in input.lines().enumerate() {
        for ch in line.chars() {
            let suspicious = ch == '\0'
                || ch == '\u{fffd}'
                || (ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'));
            if !suspicious {
                continue;
            }
            first_line.get_or_insert(line_index + 1);
            suspicious_chars += 1;
            if ch == '\0' {
                nul_chars += 1;
            } else if ch == '\u{fffd}' {
                replacement_chars += 1;
            }
        }
    }

    let suspicious_ratio = suspicious_chars.saturating_mul(100) / total_chars.max(1);
    if nul_chars == 0 && replacement_chars < 2 && (suspicious_chars < 4 || suspicious_ratio < 2) {
        return None;
    }

    Some(ContextBlobNoiseFinding {
        kind: ContextBlobNoiseKind::BinaryText,
        line: first_line,
        bytes: input.len(),
        score: (60 + suspicious_ratio).min(100),
        detail: format!(
            "binaryish_controls={suspicious_chars}, nul_chars={nul_chars}, replacement_chars={replacement_chars}"
        ),
    })
}
