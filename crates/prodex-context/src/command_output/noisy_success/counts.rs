pub(crate) fn has_nonzero_summary_count(lower: &str, words: &[&str]) -> bool {
    words.iter().any(|word| {
        lower.match_indices(word).any(|(index, matched)| {
            if let Some(count) = count_after_word(lower, index + matched.len()) {
                return count > 0;
            }
            count_before_word(lower, index).is_some_and(|count| count > 0)
        })
    })
}

pub(crate) fn has_zero_only_summary_count(lower: &str, words: &[&str]) -> bool {
    let mut saw_count = false;
    for word in words {
        for (index, matched) in lower.match_indices(word) {
            let count = count_after_word(lower, index + matched.len())
                .or_else(|| count_before_word(lower, index));
            let Some(count) = count else {
                continue;
            };
            saw_count = true;
            if count > 0 {
                return false;
            }
        }
    }
    saw_count
}

pub(crate) fn count_after_word(lower: &str, after_word_index: usize) -> Option<usize> {
    let after = lower.get(after_word_index..)?.trim_start();
    let after = if let Some(rest) = after.strip_prefix(':') {
        rest
    } else if let Some(rest) = after.strip_prefix('=') {
        rest
    } else {
        after.strip_prefix('(')?
    };
    let digits = after
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<usize>().ok())?
}

pub(crate) fn count_before_word(lower: &str, word_index: usize) -> Option<usize> {
    let before = lower
        .get(..word_index)?
        .trim_end_matches(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ':' | ';' | '('));
    let digits = before
        .chars()
        .rev()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<usize>().ok())?
}
