use super::{ContextBlobNoiseFinding, ContextBlobNoiseKind};

pub(super) fn detect_base64ish_blob_noise_supplement(
    lines: &[&str],
) -> Option<ContextBlobNoiseFinding> {
    let mut best_line = None;
    let mut best_bytes = 0usize;
    let mut best_score = 0usize;
    let mut block_start = 0usize;
    let mut block_lines = 0usize;
    let mut block_bytes = 0usize;
    let mut block_score = 0usize;

    for (index, line) in lines.iter().enumerate() {
        if let Some((bytes, score)) = longest_base64ish_span_supplement(line, 160, 16)
            && bytes > best_bytes
        {
            best_line = Some(index + 1);
            best_bytes = bytes;
            best_score = score;
        }

        let trimmed = line.trim();
        if let Some(score) = base64ish_candidate_score_supplement(trimmed, 56, 12) {
            if block_lines == 0 {
                block_start = index + 1;
            }
            block_lines += 1;
            block_bytes += trimmed.len();
            block_score = block_score.max(score);
        } else {
            if block_lines >= 4 && block_bytes >= 240 && block_bytes > best_bytes {
                best_line = Some(block_start);
                best_bytes = block_bytes;
                best_score = block_score;
            }
            block_lines = 0;
            block_bytes = 0;
            block_score = 0;
        }
    }

    if block_lines >= 4 && block_bytes >= 240 && block_bytes > best_bytes {
        best_line = Some(block_start);
        best_bytes = block_bytes;
        best_score = block_score;
    }

    (best_bytes > 0).then(|| ContextBlobNoiseFinding {
        kind: ContextBlobNoiseKind::Base64Blob,
        line: best_line,
        bytes: best_bytes,
        score: best_score,
        detail: format!("base64ish_bytes={best_bytes}"),
    })
}

fn longest_base64ish_span_supplement(
    line: &str,
    min_bytes: usize,
    min_unique_chars: usize,
) -> Option<(usize, usize)> {
    let mut best = None;
    let mut start = None;

    for (index, ch) in line.char_indices() {
        if is_base64ish_char_supplement(ch) {
            start.get_or_insert(index);
            continue;
        }
        if let Some(span_start) = start.take() {
            let candidate = &line[span_start..index];
            if let Some(score) =
                base64ish_candidate_score_supplement(candidate, min_bytes, min_unique_chars)
            {
                best = Some(match best {
                    Some((bytes, best_score)) if bytes >= candidate.len() => (bytes, best_score),
                    _ => (candidate.len(), score),
                });
            }
        }
    }

    if let Some(span_start) = start {
        let candidate = &line[span_start..];
        if let Some(score) =
            base64ish_candidate_score_supplement(candidate, min_bytes, min_unique_chars)
        {
            best = Some(match best {
                Some((bytes, best_score)) if bytes >= candidate.len() => (bytes, best_score),
                _ => (candidate.len(), score),
            });
        }
    }

    best
}

fn base64ish_candidate_score_supplement(
    candidate: &str,
    min_bytes: usize,
    min_unique_chars: usize,
) -> Option<usize> {
    let candidate = candidate.trim_matches(|ch| matches!(ch, '"' | '\'' | '`' | ',' | ';'));
    if candidate.len() < min_bytes || !candidate.is_ascii() {
        return None;
    }

    let mut base64_chars = 0usize;
    let mut upper = 0usize;
    let mut lower = 0usize;
    let mut digit = 0usize;
    let mut symbol = 0usize;
    let mut seen = [false; 128];
    for byte in candidate.bytes() {
        let ch = byte as char;
        if !is_base64ish_char_supplement(ch) {
            continue;
        }
        base64_chars += 1;
        seen[byte as usize] = true;
        if ch.is_ascii_uppercase() {
            upper += 1;
        } else if ch.is_ascii_lowercase() {
            lower += 1;
        } else if ch.is_ascii_digit() {
            digit += 1;
        } else {
            symbol += 1;
        }
    }

    let unique_chars = seen.into_iter().filter(|seen| *seen).count();
    let ratio = base64_chars.saturating_mul(100) / candidate.len().max(1);
    let classes = [upper, lower, digit, symbol]
        .into_iter()
        .filter(|count| *count > 0)
        .count();
    if ratio < 96 || unique_chars < min_unique_chars || classes < 2 {
        return None;
    }

    Some((70 + ratio.saturating_sub(96).saturating_mul(8)).min(100))
}

fn is_base64ish_char_supplement(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '_' | '-' | '=')
}
