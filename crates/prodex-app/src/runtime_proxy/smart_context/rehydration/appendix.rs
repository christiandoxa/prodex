//! Exact-appendix rendering and rehydration budget helpers.

use super::*;

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_line_excerpt(
    lines: &[&str],
    start: usize,
    end: usize,
) -> Option<String> {
    if start == 0 || start > lines.len() || end < start {
        return None;
    }
    let end = end.min(lines.len());
    Some(lines[start - 1..end].join("\n"))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_render_exact_appendix(
    label: &str,
    ranges: Vec<RuntimeSmartContextExactAppendixRange>,
) -> Option<(String, usize)> {
    runtime_proxy_crate::smart_context_render_exact_appendix(
        label,
        ranges
            .into_iter()
            .map(
                |range| runtime_proxy_crate::SmartContextExactAppendixRange {
                    reference: range.reference,
                    body: range.body,
                },
            )
            .collect(),
    )
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_compact_line_refs_if_shorter(
    refs: &[String],
) -> String {
    runtime_proxy_crate::smart_context_compact_line_refs_if_shorter(refs)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_render_scored_exact_appendix(
    label: &str,
    ranges: Vec<RuntimeSmartContextExactAppendixRange>,
    max_exact_ranges: usize,
    score: impl Fn(&RuntimeSmartContextExactAppendixRange) -> usize,
) -> Option<(String, usize)> {
    let ranges = ranges
        .into_iter()
        .filter(|range| !range.body.trim().is_empty())
        .collect::<Vec<_>>();
    let range_count = ranges.len();
    if range_count == 0 {
        return None;
    }
    let max_exact_ranges = max_exact_ranges.max(1);
    if range_count <= max_exact_ranges {
        return runtime_smart_context_render_exact_appendix(label, ranges);
    }

    let mut ranked = ranges
        .iter()
        .enumerate()
        .map(|(index, range)| (index, score(range)))
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(index, score)| (std::cmp::Reverse(*score), *index));
    let selected_indexes = ranked
        .iter()
        .take(max_exact_ranges)
        .map(|(index, _)| *index)
        .collect::<BTreeSet<_>>();

    let mut selected = Vec::new();
    let mut fallback_refs = Vec::new();
    for (index, range) in ranges.into_iter().enumerate() {
        if selected_indexes.contains(&index) {
            selected.push(range);
        } else {
            fallback_refs.push(range.reference);
        }
    }
    let selected_count = selected.len();

    let Some((mut appendix, _)) = runtime_smart_context_render_exact_appendix(label, selected)
    else {
        let refs = runtime_smart_context_compact_line_refs_if_shorter(&fallback_refs);
        return Some((format!("{label}\nrefs: {refs}"), selected_count));
    };
    if !fallback_refs.is_empty() {
        appendix.push('\n');
        appendix.push_str("refs: ");
        appendix.push_str(&runtime_smart_context_compact_line_refs_if_shorter(
            &fallback_refs,
        ));
    }
    Some((appendix, selected_count))
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_render_budgeted_scored_exact_appendix(
    label: &str,
    ranges: Vec<RuntimeSmartContextExactAppendixRange>,
    max_exact_ranges: usize,
    token_budget: usize,
    score: impl Fn(&RuntimeSmartContextExactAppendixRange) -> usize,
) -> Option<(String, usize, usize)> {
    if token_budget == usize::MAX {
        let (appendix, range_count) = runtime_smart_context_render_scored_exact_appendix(
            label,
            ranges,
            max_exact_ranges,
            score,
        )?;
        let token_cost = runtime_smart_context_estimated_tokens_usize(appendix.len());
        return Some((appendix, range_count, token_cost));
    }
    if token_budget == 0 {
        return None;
    }

    let ranges = ranges
        .into_iter()
        .filter(|range| !range.body.trim().is_empty())
        .collect::<Vec<_>>();
    if ranges.is_empty() {
        return None;
    }

    let max_exact_ranges = max_exact_ranges.max(1);
    let mut ranked = ranges
        .iter()
        .enumerate()
        .map(|(index, range)| (index, score(range)))
        .collect::<Vec<_>>();
    ranked.sort_by_key(|(index, score)| (std::cmp::Reverse(*score), *index));

    let mut selected_indexes = Vec::new();
    let mut estimated_tokens = runtime_smart_context_estimated_tokens_usize(label.len());
    for (index, _) in ranked {
        if selected_indexes.len() >= max_exact_ranges {
            break;
        }
        let next_cost = runtime_smart_context_exact_appendix_range_token_cost(&ranges[index]);
        if estimated_tokens.saturating_add(next_cost) > token_budget {
            continue;
        }
        selected_indexes.push(index);
        estimated_tokens = estimated_tokens.saturating_add(next_cost);
    }
    if selected_indexes.is_empty() {
        return None;
    }
    selected_indexes.sort_unstable();
    let selected = selected_indexes
        .into_iter()
        .map(|index| ranges[index].clone())
        .collect::<Vec<_>>();
    let selected_count = selected.len();
    let (appendix, _) = runtime_smart_context_render_exact_appendix(label, selected)?;
    let actual_tokens = runtime_smart_context_estimated_tokens_usize(appendix.len());
    (actual_tokens <= token_budget).then_some((appendix, selected_count, actual_tokens))
}

fn runtime_smart_context_exact_appendix_range_token_cost(
    range: &RuntimeSmartContextExactAppendixRange,
) -> usize {
    runtime_smart_context_estimated_tokens_usize(
        range
            .reference
            .len()
            .saturating_add(range.body.len())
            .saturating_add(2),
    )
}

fn runtime_smart_context_estimated_tokens_usize(byte_len: usize) -> usize {
    runtime_proxy_crate::smart_context_estimate_tokens_from_body_bytes(byte_len)
        .try_into()
        .unwrap_or(usize::MAX)
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_consume_rehydrate_budget(
    remaining_tokens: &mut usize,
    used_tokens: usize,
) {
    if *remaining_tokens != usize::MAX {
        *remaining_tokens = remaining_tokens.saturating_sub(used_tokens);
    }
}
