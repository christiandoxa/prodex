use std::collections::BTreeMap;

pub fn smart_context_path_aliases(text: &str) -> Vec<(String, String)> {
    let mut counts = BTreeMap::<String, usize>::new();
    for token in text.split_whitespace() {
        let token = token.trim_matches(|ch: char| {
            ch.is_ascii_punctuation() && !matches!(ch, '/' | '_' | '-' | '.')
        });
        if let Some(prefix) = smart_context_repeated_path_prefix(token) {
            *counts.entry(prefix).or_default() += 1;
        }
    }
    let mut candidates = counts
        .into_iter()
        .filter(|(prefix, count)| *count >= 2 && prefix.len() > 8)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .len()
            .saturating_mul(right.1)
            .cmp(&left.0.len().saturating_mul(left.1))
    });
    candidates
        .into_iter()
        .take(4)
        .enumerate()
        .filter_map(|(index, (prefix, count))| {
            let alias = if index == 0 {
                "$R".to_string()
            } else {
                format!("$P{index}")
            };
            let saved = prefix
                .len()
                .saturating_sub(alias.len())
                .saturating_mul(count);
            let cost = alias.len().saturating_add(prefix.len()).saturating_add(2);
            (saved > cost).then_some((alias, prefix))
        })
        .collect()
}

pub(in crate::smart_context) fn smart_context_repeated_path_prefix(token: &str) -> Option<String> {
    if !token.starts_with('/') {
        return None;
    }
    for marker in [
        "/crates/",
        "/src/",
        "/tests/",
        "/test/",
        "/dist/",
        "/target/",
        "/apps/",
        "/packages/",
    ] {
        if let Some(index) = token.find(marker) {
            let prefix = &token[..index];
            if prefix.matches('/').count() >= 2 {
                return Some(prefix.to_string());
            }
        }
    }
    token
        .rsplit_once('/')
        .map(|(prefix, _)| prefix)
        .filter(|prefix| prefix.matches('/').count() >= 3)
        .map(str::to_string)
}
