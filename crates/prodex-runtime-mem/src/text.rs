pub(crate) fn runtime_mem_first_useful_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

pub(crate) fn runtime_mem_truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

pub(crate) fn runtime_mem_approx_token_count(text: &str) -> usize {
    text.split_whitespace().count()
}
