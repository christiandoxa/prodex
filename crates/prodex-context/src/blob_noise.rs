use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

use crate::{
    command_lines, count_text_lines, normalize_command_output, parse_file_list_entry_line,
    parse_rg_json_match_line, parse_search_match_line,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextBlobNoiseKind {
    Base64Blob,
    MinifiedJsJson,
    LockfileOrVendor,
    BinaryText,
    RepeatedPathFlood,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextBlobNoiseFinding {
    pub kind: ContextBlobNoiseKind,
    pub line: Option<usize>,
    pub bytes: usize,
    pub score: usize,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ContextBlobNoiseReport {
    pub bytes: usize,
    pub lines: usize,
    pub findings: Vec<ContextBlobNoiseFinding>,
}

impl ContextBlobNoiseReport {
    pub fn is_noise(&self) -> bool {
        !self.findings.is_empty()
    }

    pub fn has_kind(&self, kind: ContextBlobNoiseKind) -> bool {
        self.findings.iter().any(|finding| finding.kind == kind)
    }
}

pub fn detect_context_blob_noise(input: &str) -> ContextBlobNoiseReport {
    detect_context_blob_noise_inner(None, input)
}

pub fn detect_context_blob_noise_for_path(path: &Path, input: &str) -> ContextBlobNoiseReport {
    detect_context_blob_noise_inner(Some(path), input)
}

pub fn is_context_blob_noise(input: &str) -> bool {
    detect_context_blob_noise(input).is_noise()
}

fn detect_context_blob_noise_inner(path: Option<&Path>, input: &str) -> ContextBlobNoiseReport {
    let normalized = normalize_command_output(input);
    let lines = command_lines(&normalized);
    let mut findings = Vec::new();

    if path.is_some_and(is_lockfile_or_vendor_path) {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: None,
            bytes: input.len(),
            score: input.len().min(usize::MAX / 2),
            detail: "path looks like generated dependency/vendor content".to_string(),
        });
    }

    if input.chars().any(|ch| ch == '\0' || ch == '\u{fffd}') {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::BinaryText,
            line: None,
            bytes: input.len(),
            score: input.len(),
            detail: "text contains binary replacement or NUL characters".to_string(),
        });
    }

    for (index, line) in lines.iter().enumerate() {
        let line_number = Some(index + 1);
        let trimmed = line.trim();
        if trimmed.len() >= 512 && looks_like_base64_blob(trimmed) {
            findings.push(ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::Base64Blob,
                line: line_number,
                bytes: trimmed.len(),
                score: trimmed.len(),
                detail: "long high-entropy base64-like line".to_string(),
            });
        }
        if trimmed.len() >= 800 && looks_like_minified_js_json(trimmed) {
            findings.push(ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::MinifiedJsJson,
                line: line_number,
                bytes: trimmed.len(),
                score: trimmed.len(),
                detail: "long minified JSON/JavaScript-like line".to_string(),
            });
        }
    }

    if let Some((line, count)) = repeated_path_flood(&lines) {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::RepeatedPathFlood,
            line: Some(line),
            bytes: input.len(),
            score: count,
            detail: format!("{count} path-like lines detected"),
        });
    }

    add_context_blob_noise_supplemental_findings(path, input, &lines, &mut findings);

    ContextBlobNoiseReport {
        bytes: input.len(),
        lines: count_text_lines(&normalized),
        findings,
    }
}

fn is_lockfile_or_vendor_path(path: &Path) -> bool {
    let rendered = path.display().to_string();
    let normalized = rendered.replace('\\', "/").to_ascii_lowercase();
    normalized.ends_with("cargo.lock")
        || normalized.ends_with("package-lock.json")
        || normalized.ends_with("pnpm-lock.yaml")
        || normalized.ends_with("yarn.lock")
        || normalized.contains("/node_modules/")
        || normalized.contains("/vendor/")
        || normalized.contains("/target/")
}

fn looks_like_base64_blob(line: &str) -> bool {
    let base64_chars = line
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '=' | '-' | '_'))
        .count();
    base64_chars.saturating_mul(100) >= line.len().saturating_mul(95)
        && line.chars().any(|ch| ch.is_ascii_digit())
        && line.chars().any(|ch| ch.is_ascii_uppercase())
        && line.chars().any(|ch| ch.is_ascii_lowercase())
}

fn looks_like_minified_js_json(line: &str) -> bool {
    let punctuation = line
        .chars()
        .filter(|ch| matches!(ch, '{' | '}' | '[' | ']' | ':' | ',' | ';'))
        .count();
    let spaces = line.chars().filter(|ch| ch.is_whitespace()).count();
    punctuation >= 32 && spaces.saturating_mul(80) < line.len()
}

fn repeated_path_flood(lines: &[&str]) -> Option<(usize, usize)> {
    let mut first_path_line = None;
    let mut path_lines = 0usize;
    let mut non_empty = 0usize;
    for (index, line) in lines.iter().enumerate() {
        if !line.trim().is_empty() {
            non_empty += 1;
        }
        if parse_file_list_entry_line(line).is_some() || parse_search_match_line(line).is_some() {
            path_lines += 1;
            first_path_line.get_or_insert(index + 1);
        }
    }

    if path_lines >= 200 && path_lines.saturating_mul(100) >= non_empty.saturating_mul(80) {
        first_path_line.map(|line| (line, path_lines))
    } else {
        None
    }
}

fn add_context_blob_noise_supplemental_findings(
    path: Option<&Path>,
    input: &str,
    lines: &[&str],
    findings: &mut Vec<ContextBlobNoiseFinding>,
) {
    if let Some(finding) = detect_binaryish_text_noise_supplement(input) {
        push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_base64ish_blob_noise_supplement(lines) {
        push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_minified_js_json_noise_supplement(input, lines) {
        push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_lockfile_or_vendor_noise_supplement(path, input, lines) {
        push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_repeated_path_flood_noise_supplement(input, lines) {
        push_context_blob_noise_finding(findings, finding);
    }
}

fn push_context_blob_noise_finding(
    findings: &mut Vec<ContextBlobNoiseFinding>,
    finding: ContextBlobNoiseFinding,
) {
    if !findings
        .iter()
        .any(|existing| existing.kind == finding.kind)
    {
        findings.push(finding);
    }
}

fn detect_binaryish_text_noise_supplement(input: &str) -> Option<ContextBlobNoiseFinding> {
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

fn detect_base64ish_blob_noise_supplement(lines: &[&str]) -> Option<ContextBlobNoiseFinding> {
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

fn detect_minified_js_json_noise_supplement(
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

fn detect_lockfile_or_vendor_noise_supplement(
    path: Option<&Path>,
    input: &str,
    lines: &[&str],
) -> Option<ContextBlobNoiseFinding> {
    if let Some(path) = path
        && let Some(detail) = context_lockfile_or_vendor_path_detail_supplement(path)
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: None,
            bytes: input.len(),
            score: 100,
            detail,
        });
    }

    let cargo_packages = lines
        .iter()
        .filter(|line| line.trim() == "[[package]]")
        .count();
    let cargo_checksums = lines
        .iter()
        .filter(|line| line.trim_start().starts_with("checksum = "))
        .count();
    if cargo_packages >= 4 && cargo_checksums >= 2 {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 95,
            detail: format!("cargo_lock_packages={cargo_packages}"),
        });
    }

    let lower = input.to_ascii_lowercase();
    if lower.contains("\"lockfileversion\"")
        && (lower.contains("\"packages\"") || lower.contains("\"dependencies\""))
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 95,
            detail: "npm_lockfile_json".to_string(),
        });
    }

    if lower.contains("# yarn lockfile")
        || lower.contains("pnpm-lock.yaml")
        || lower.contains("lockfileversion:")
            && (lower.contains("importers:") || lower.contains("packages:"))
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: Some(1),
            bytes: input.len(),
            score: 90,
            detail: "package_manager_lockfile".to_string(),
        });
    }

    let non_empty_lines = lines.iter().filter(|line| !line.trim().is_empty()).count();
    let mut vendor_path_lines = 0usize;
    let mut first_vendor_line = None;
    for (index, line) in lines.iter().enumerate() {
        if context_line_has_vendor_path_supplement(line) {
            vendor_path_lines += 1;
            first_vendor_line.get_or_insert(index + 1);
        }
    }
    if vendor_path_lines >= 8
        && vendor_path_lines.saturating_mul(100) >= non_empty_lines.max(1) * 40
    {
        return Some(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: first_vendor_line,
            bytes: input.len(),
            score: 85,
            detail: format!("vendor_path_lines={vendor_path_lines}"),
        });
    }

    None
}

fn context_lockfile_or_vendor_path_detail_supplement(path: &Path) -> Option<String> {
    let normalized = path.display().to_string().replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    let file_name = lower.rsplit('/').next().unwrap_or(lower.as_str());
    if matches!(
        file_name,
        "cargo.lock"
            | "package-lock.json"
            | "npm-shrinkwrap.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "bun.lock"
            | "bun.lockb"
            | "poetry.lock"
            | "pipfile.lock"
            | "gemfile.lock"
            | "composer.lock"
            | "go.sum"
    ) {
        return Some(format!("lockfile_path={normalized}"));
    }

    [
        "/node_modules/",
        "/vendor/",
        "/third_party/",
        "/.cargo/registry/",
        "/.pnpm/",
        "/.yarn/cache/",
    ]
    .into_iter()
    .find(|marker| {
        lower.contains(marker) || lower.trim_start_matches("./").starts_with(&marker[1..])
    })
    .map(|_| format!("vendor_path={normalized}"))
}

fn context_line_has_vendor_path_supplement(line: &str) -> bool {
    let lower = line.replace('\\', "/").to_ascii_lowercase();
    [
        "/node_modules/",
        "node_modules/",
        "/vendor/",
        "vendor/",
        "/third_party/",
        "third_party/",
        "/.cargo/registry/",
        ".cargo/registry/",
        "/.pnpm/",
        ".pnpm/",
        "/.yarn/cache/",
        ".yarn/cache/",
    ]
    .into_iter()
    .any(|marker| lower.contains(marker))
}

fn detect_repeated_path_flood_noise_supplement(
    input: &str,
    lines: &[&str],
) -> Option<ContextBlobNoiseFinding> {
    let mut path_counts = BTreeMap::<String, usize>::new();
    let mut prefix_counts = BTreeMap::<String, usize>::new();
    let mut first_path_line = BTreeMap::<String, usize>::new();
    let mut path_lines = 0usize;

    for (index, line) in lines.iter().enumerate() {
        let Some(path) = context_noise_extract_path_key_supplement(line) else {
            continue;
        };
        path_lines += 1;
        first_path_line.entry(path.clone()).or_insert(index + 1);
        *path_counts.entry(path.clone()).or_default() += 1;
        *prefix_counts
            .entry(context_noise_path_prefix_supplement(&path))
            .or_default() += 1;
    }

    if path_lines < 16 {
        return None;
    }

    let (top_path, top_path_count) = path_counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))?;
    let top_prefix_count = prefix_counts.values().copied().max().unwrap_or(0);
    let unique_paths = path_counts.len();
    let exact_flood = *top_path_count >= 8 && top_path_count.saturating_mul(100) >= path_lines * 35;
    let low_variety_flood = path_lines >= 24 && unique_paths.saturating_mul(100) <= path_lines * 20;
    let prefix_flood =
        top_prefix_count >= 24 && top_prefix_count.saturating_mul(100) >= path_lines * 70;

    if !exact_flood && !low_variety_flood && !prefix_flood {
        return None;
    }

    Some(ContextBlobNoiseFinding {
        kind: ContextBlobNoiseKind::RepeatedPathFlood,
        line: first_path_line.get(top_path.as_str()).copied(),
        bytes: input.len(),
        score: if exact_flood { 92 } else { 84 },
        detail: format!(
            "path_lines={path_lines}, unique_paths={unique_paths}, top_path_count={top_path_count}, top_path={top_path}"
        ),
    })
}

fn context_noise_extract_path_key_supplement(line: &str) -> Option<String> {
    if let Some(search_match) =
        parse_search_match_line(line).or_else(|| parse_rg_json_match_line(line))
    {
        return context_noise_normalize_path_token_supplement(&search_match.path);
    }

    if let Some(path) = parse_file_list_entry_line(line) {
        return context_noise_normalize_path_token_supplement(&path);
    }

    line.split_whitespace()
        .find_map(context_noise_normalize_path_token_supplement)
}

pub(crate) fn context_noise_normalize_path_token_supplement(token: &str) -> Option<String> {
    let token = token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
        )
    });
    if token.contains("://") {
        return None;
    }

    let normalized = token.replace('\\', "/");
    let stripped = context_noise_strip_path_location_suffix_supplement(&normalized);
    let stripped = stripped
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches('/');
    if stripped.len() < 3 || !stripped.contains('/') || stripped.contains(' ') {
        return None;
    }

    Some(stripped.to_string())
}

pub(crate) fn context_noise_strip_path_location_suffix_supplement(path: &str) -> &str {
    let mut value = path;
    for _ in 0..2 {
        let Some((head, tail)) = value.rsplit_once(':') else {
            break;
        };
        if tail.is_empty() || !tail.chars().all(|ch| ch.is_ascii_digit()) {
            break;
        }
        value = head;
    }
    value
}

fn context_noise_path_prefix_supplement(path: &str) -> String {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .take(3)
        .collect::<Vec<_>>();
    if segments.len() <= 1 {
        path.to_string()
    } else {
        segments.join("/")
    }
}
