use std::collections::BTreeMap;

use crate::{parse_file_list_entry_line, parse_rg_json_match_line, parse_search_match_line};

use super::{ContextBlobNoiseFinding, ContextBlobNoiseKind};

pub(super) fn detect_repeated_path_flood_noise_supplement(
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
