use super::{ContextBlobNoiseFinding, ContextBlobNoiseKind};

pub(super) fn detect_minified_js_json_noise_supplement(
    input: &str,
    lines: &[&str],
) -> Option<ContextBlobNoiseFinding> {
    let trimmed = input.trim();
    if trimmed.len() < 512 {
        return None;
    }

    let non_empty_lines = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let max_line_bytes = lines
        .iter()
        .map(|line| line.trim().len())
        .max()
        .unwrap_or(0);
    if non_empty_lines > 4 && max_line_bytes < 512 {
        return None;
    }

    let chars = trimmed.chars().count();
    let whitespace = trimmed.chars().filter(|ch| ch.is_whitespace()).count();
    if whitespace.saturating_mul(100) > chars.max(1).saturating_mul(5) {
        return None;
    }

    let punctuation = trimmed
        .chars()
        .filter(|ch| matches!(ch, '{' | '}' | '[' | ']' | '(' | ')' | ':' | ',' | ';'))
        .count();
    if punctuation.saturating_mul(100) < chars.max(1).saturating_mul(8) {
        return None;
    }

    let (detail, score) = if looks_like_minified_json_supplement(trimmed) {
        ("minified_json", 92)
    } else if looks_like_minified_javascript_supplement(trimmed) {
        ("minified_javascript", 88)
    } else {
        return None;
    };

    Some(ContextBlobNoiseFinding {
        kind: ContextBlobNoiseKind::MinifiedJsJson,
        line: Some(1),
        bytes: trimmed.len(),
        score,
        detail: format!("{detail}, max_line_bytes={max_line_bytes}"),
    })
}

fn looks_like_minified_json_supplement(input: &str) -> bool {
    (input.starts_with('{') && input.ends_with('}')
        || input.starts_with('[') && input.ends_with(']'))
        && input.matches(':').count() >= 8
        && input.matches(',').count() >= 8
        && input.matches('"').count() >= 16
}

fn looks_like_minified_javascript_supplement(input: &str) -> bool {
    let function_markers = count_substring_supplement(input, "function(")
        + count_substring_supplement(input, "=>")
        + count_substring_supplement(input, "module.exports")
        + count_substring_supplement(input, "exports.");
    let semicolons = input.matches(';').count();
    let braces = input.matches('{').count() + input.matches('}').count();
    let parens = input.matches('(').count() + input.matches(')').count();

    function_markers >= 2 && (semicolons >= 8 || braces + parens >= 32)
}

fn count_substring_supplement(input: &str, needle: &str) -> usize {
    input.match_indices(needle).count()
}
