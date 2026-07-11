pub(super) fn runtime_smart_context_common_prefix_boundary_len(left: &str, right: &str) -> usize {
    let max_len = left.len().min(right.len());
    let mut len = 0usize;
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    while len < max_len && left_bytes[len] == right_bytes[len] {
        len += 1;
    }
    while len > 0 && (!left.is_char_boundary(len) || !right.is_char_boundary(len)) {
        len -= 1;
    }
    len
}

pub(super) fn runtime_smart_context_common_suffix_boundary_len(
    left: &str,
    right: &str,
    prefix_bytes: usize,
) -> usize {
    let max_len = left.len().min(right.len()).saturating_sub(prefix_bytes);
    let mut len = 0usize;
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    while len < max_len && left_bytes[left.len() - len - 1] == right_bytes[right.len() - len - 1] {
        len += 1;
    }
    while len > 0
        && (!left.is_char_boundary(left.len() - len) || !right.is_char_boundary(right.len() - len))
    {
        len -= 1;
    }
    len
}

pub(super) fn runtime_smart_context_tool_args_preview_max_chars(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
) -> usize {
    match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => 160,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => 240,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => 360,
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => 480,
    }
}
