use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn runtime_smart_context_try_surgical_rehydrate_critical_ranges(
    value: &serde_json::Value,
    shared: &RuntimeRotationProxyShared,
    request_body: &[u8],
    exactness: &runtime_proxy_crate::SmartContextExactnessGuard,
    unresolved_rehydrate_refs: &[String],
    stats: &RuntimeSmartContextTransformStats,
) -> Option<(Vec<u8>, RuntimeSmartContextTransformStats)> {
    with_runtime_smart_context_artifacts(shared, |store| {
        let mut repaired_value = value.clone();
        let mut repaired_stats = stats.clone();
        if runtime_smart_context_rehydrate_lost_critical_ranges(
            &mut repaired_value,
            store,
            &mut repaired_stats,
        ) == 0
        {
            return None;
        }

        let repaired_body = serde_json::to_vec(&repaired_value).ok()?;
        let regression_check = runtime_smart_context_regression_self_check(
            request_body,
            &repaired_body,
            exactness.clone(),
            unresolved_rehydrate_refs.to_vec(),
        );
        let critical_signal_check =
            runtime_smart_context_critical_signal_self_check(request_body, &repaired_body);
        runtime_smart_context_fallback_exact_reason(
            &regression_check,
            critical_signal_check,
            &repaired_stats,
        )
        .is_none()
        .then_some((repaired_body, repaired_stats))
    })
    .flatten()
}

fn runtime_smart_context_rehydrate_lost_critical_ranges(
    value: &mut serde_json::Value,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    if runtime_smart_context_value_is_static_context_item(value) {
        return 0;
    }
    match value {
        serde_json::Value::String(text) => {
            runtime_smart_context_rehydrate_lost_critical_ranges_in_text(text, store, stats)
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|item| runtime_smart_context_rehydrate_lost_critical_ranges(item, store, stats))
            .sum(),
        serde_json::Value::Object(object) => {
            let mut count = 0usize;
            for (key, item) in object {
                if runtime_smart_context_static_prompt_field_key(key) {
                    continue;
                }
                count = count.saturating_add(runtime_smart_context_rehydrate_lost_critical_ranges(
                    item, store, stats,
                ));
            }
            count
        }
        _ => 0,
    }
}

fn runtime_smart_context_rehydrate_lost_critical_ranges_in_text(
    text: &mut String,
    store: &RuntimeSmartContextArtifactStore,
    stats: &mut RuntimeSmartContextTransformStats,
) -> usize {
    let ids = runtime_smart_context_collect_artifact_refs(&serde_json::Value::String(text.clone()))
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return 0;
    }

    let mut next = text.clone();
    let mut rehydrated_ranges = 0usize;
    for id in ids {
        let Some(artifact_text) = store.get_text(&id) else {
            continue;
        };
        let artifact_line_index = store.line_index(&id);
        let Some((appendix, range_count)) = runtime_smart_context_missing_critical_range_appendix(
            &id,
            &artifact_text,
            artifact_line_index,
            &next,
        ) else {
            continue;
        };
        if !next.ends_with('\n') {
            next.push('\n');
        }
        next.push('\n');
        next.push_str(&appendix);
        rehydrated_ranges = rehydrated_ranges.saturating_add(range_count);
    }

    if rehydrated_ranges > 0 {
        *text = next;
        stats.rehydrated_refs = stats.rehydrated_refs.saturating_add(rehydrated_ranges);
    }
    rehydrated_ranges
}

enum RuntimeSmartContextIndexedCriticalAppendix {
    Found(String, usize),
    NoLoss,
    Unusable,
}

pub(super) fn runtime_smart_context_missing_critical_range_appendix(
    artifact_id: &str,
    artifact_text: &str,
    artifact_line_index: Option<&RuntimeSmartContextArtifactLineIndex>,
    current_text: &str,
) -> Option<(String, usize)> {
    if let Some(line_index) = artifact_line_index {
        match runtime_smart_context_missing_indexed_critical_range_appendix(
            artifact_id,
            line_index,
            current_text,
        ) {
            RuntimeSmartContextIndexedCriticalAppendix::Found(appendix, range_count) => {
                return Some((appendix, range_count));
            }
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss => return None,
            RuntimeSmartContextIndexedCriticalAppendix::Unusable => {}
        }
    }

    let ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        artifact_text,
        current_text,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 1,
            max_ranges: SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
            max_range_lines: 6,
        },
    );
    if ranges.is_empty() {
        return None;
    }

    let mut exact_ranges = Vec::new();
    for range in ranges {
        let exact = runtime_smart_context_rehydrated_artifact_text(
            artifact_text,
            Some(RuntimeSmartContextLineRange {
                start: range.start,
                end: range.end,
            }),
        )?;
        if exact.trim().is_empty() {
            continue;
        }
        exact_ranges.push(RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref(artifact_id, range.start, range.end),
            body: exact,
        });
    }

    runtime_smart_context_render_exact_appendix(SMART_CONTEXT_LABEL_CRITICAL_EXACT, exact_ranges)
}

fn runtime_smart_context_missing_indexed_critical_range_appendix(
    artifact_id: &str,
    line_index: &RuntimeSmartContextArtifactLineIndex,
    current_text: &str,
) -> RuntimeSmartContextIndexedCriticalAppendix {
    if line_index.critical_ranges.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let mut synthetic = String::new();
    let mut synthetic_line_to_range = Vec::new();
    for (range_index, range) in line_index.critical_ranges.iter().enumerate() {
        if !runtime_smart_context_artifact_line_index_range_valid(range) {
            return RuntimeSmartContextIndexedCriticalAppendix::Unusable;
        }
        for line in range.text.lines() {
            if !synthetic.is_empty() {
                synthetic.push('\n');
            }
            synthetic.push_str(line);
            synthetic_line_to_range.push(range_index);
        }
    }

    if synthetic_line_to_range.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let lost_synthetic_ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        &synthetic,
        current_text,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 0,
            max_ranges: SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
            max_range_lines: 1,
        },
    );
    if lost_synthetic_ranges.is_empty() {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    }

    let mut selected = BTreeSet::<usize>::new();
    for lost_range in lost_synthetic_ranges {
        for synthetic_line in lost_range.start..=lost_range.end {
            let Some(range_index) = synthetic_line_to_range.get(synthetic_line - 1) else {
                return RuntimeSmartContextIndexedCriticalAppendix::Unusable;
            };
            selected.insert(*range_index);
            if selected.len() >= SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES {
                break;
            }
        }
        if selected.len() >= SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES {
            break;
        }
    }

    let mut exact_ranges = Vec::new();
    for range_index in &selected {
        let range = &line_index.critical_ranges[*range_index];
        if range.text.trim().is_empty() {
            continue;
        }
        exact_ranges.push(RuntimeSmartContextExactAppendixRange {
            reference: runtime_smart_context_artifact_line_ref(artifact_id, range.start, range.end),
            body: range.text.clone(),
        });
    }

    let Some((appendix, range_count)) = runtime_smart_context_render_exact_appendix(
        SMART_CONTEXT_LABEL_CRITICAL_EXACT,
        exact_ranges,
    ) else {
        return if line_index.complete {
            RuntimeSmartContextIndexedCriticalAppendix::NoLoss
        } else {
            RuntimeSmartContextIndexedCriticalAppendix::Unusable
        };
    };

    RuntimeSmartContextIndexedCriticalAppendix::Found(appendix, range_count)
}

pub(super) fn runtime_smart_context_artifact_line_index_range_valid(
    range: &RuntimeSmartContextArtifactLineRange,
) -> bool {
    range.start > 0
        && range.end >= range.start
        && range.byte_len == range.text.len()
        && range.content_hash == runtime_proxy_crate::smart_context_hash_text(&range.text)
}

pub(super) fn runtime_smart_context_line_excerpt(
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

pub(super) fn runtime_smart_context_render_exact_appendix(
    label: &str,
    ranges: Vec<RuntimeSmartContextExactAppendixRange>,
) -> Option<(String, usize)> {
    let range_count = ranges
        .iter()
        .filter(|range| !range.body.trim().is_empty())
        .count();
    if range_count == 0 {
        return None;
    }
    let mut rendered = vec![label.to_string()];
    let mut seen =
        BTreeMap::<(String, usize), Vec<RuntimeSmartContextSeenExactAppendixBody>>::new();
    for range in runtime_smart_context_merge_exact_appendix_ranges(ranges) {
        if range.body.trim().is_empty() {
            continue;
        }
        rendered.push(runtime_smart_context_render_exact_appendix_range(
            &range, &mut seen,
        ));
    }
    Some((rendered.join("\n"), range_count))
}

pub(super) fn runtime_smart_context_render_scored_exact_appendix(
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

pub(super) fn runtime_smart_context_render_budgeted_scored_exact_appendix(
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

pub(super) fn runtime_smart_context_consume_rehydrate_budget(
    remaining_tokens: &mut usize,
    used_tokens: usize,
) {
    if *remaining_tokens != usize::MAX {
        *remaining_tokens = remaining_tokens.saturating_sub(used_tokens);
    }
}

fn runtime_smart_context_merge_exact_appendix_ranges(
    ranges: Vec<RuntimeSmartContextExactAppendixRange>,
) -> Vec<RuntimeSmartContextExactAppendixRange> {
    let mut merged = Vec::<RuntimeSmartContextExactAppendixRange>::new();
    for range in ranges {
        let Some((range_base, range_lines)) =
            runtime_smart_context_parse_artifact_line_ref(&range.reference)
        else {
            merged.push(range);
            continue;
        };
        let Some(last) = merged.last_mut() else {
            merged.push(range);
            continue;
        };
        let Some((last_base, last_lines)) =
            runtime_smart_context_parse_artifact_line_ref(&last.reference)
        else {
            merged.push(range);
            continue;
        };
        if last_base != range_base
            || range_lines.start > last_lines.end.saturating_add(1)
            || range_lines.end < last_lines.start
        {
            merged.push(range);
            continue;
        }

        let overlap = if range_lines.start <= last_lines.end {
            last_lines
                .end
                .saturating_sub(range_lines.start)
                .saturating_add(1)
        } else {
            0
        };
        let next_line_count = range.body.lines().count();
        if overlap >= next_line_count && range_lines.end > last_lines.end {
            merged.push(range);
            continue;
        }
        let next_lines = range.body.lines().collect::<Vec<_>>();
        if overlap < next_lines.len() {
            if !last.body.is_empty() {
                last.body.push('\n');
            }
            last.body.push_str(&next_lines[overlap..].join("\n"));
        }
        let end = last_lines.end.max(range_lines.end);
        last.reference = format!("{last_base}#L{}-L{end}", last_lines.start);
    }
    merged
}

fn runtime_smart_context_parse_artifact_line_ref(
    reference: &str,
) -> Option<(String, RuntimeSmartContextLineRange)> {
    let (base, range) = reference.rsplit_once("#L")?;
    let (start, end) = range.split_once("-L")?;
    Some((
        base.to_string(),
        RuntimeSmartContextLineRange {
            start: start.parse().ok()?,
            end: end.parse().ok()?,
        },
    ))
}

pub(super) fn runtime_smart_context_compact_line_refs_if_shorter(refs: &[String]) -> String {
    let joined = refs.join(",");
    let Some((base, ranges)) = runtime_smart_context_line_refs_same_base(refs) else {
        return joined;
    };
    if ranges.len() < 2 {
        return joined;
    }
    let compact = format!(
        "{base}#{}",
        ranges
            .iter()
            .map(|range| format!("L{}-L{}", range.start, range.end))
            .collect::<Vec<_>>()
            .join(",")
    );
    if compact.len() < joined.len()
        && runtime_smart_context_parse_non_alias_artifact_reference(&compact)
            .is_some_and(|reference| reference.line_ranges.len() == ranges.len())
    {
        compact
    } else {
        joined
    }
}

fn runtime_smart_context_line_refs_same_base(
    refs: &[String],
) -> Option<(String, Vec<RuntimeSmartContextLineRange>)> {
    let mut base: Option<String> = None;
    let mut ranges = Vec::new();
    for reference in refs {
        let (next_base, range) = runtime_smart_context_parse_artifact_line_ref(reference)?;
        if let Some(base) = base.as_ref() {
            if base != &next_base {
                return None;
            }
        } else {
            base = Some(next_base);
        }
        ranges.push(range);
    }
    Some((base?, ranges))
}

fn runtime_smart_context_render_exact_appendix_range(
    range: &RuntimeSmartContextExactAppendixRange,
    seen: &mut BTreeMap<(String, usize), Vec<RuntimeSmartContextSeenExactAppendixBody>>,
) -> String {
    let content_hash = runtime_proxy_crate::smart_context_hash_text(&range.body);
    let byte_len = range.body.len();
    let key = (content_hash.clone(), byte_len);
    if let Some(entries) = seen.get_mut(&key)
        && let Some(existing) = entries.iter_mut().find(|entry| entry.body == range.body)
    {
        let marker = format!(
            "[psc exdup h={content_hash} b={byte_len} refs={}]",
            runtime_smart_context_compact_line_refs_if_shorter(&existing.refs)
        );
        let candidate = format!("{}\n{marker}", range.reference);
        let original = format!("{}\n{}", range.reference, range.body);
        existing.refs.push(range.reference.clone());
        if candidate.len() < original.len() {
            return candidate;
        }
        return original;
    }
    seen.entry(key)
        .or_default()
        .push(RuntimeSmartContextSeenExactAppendixBody {
            body: range.body.clone(),
            refs: vec![range.reference.clone()],
        });
    format!("{}\n{}", range.reference, range.body)
}

pub(super) fn runtime_smart_context_labeled_section_body<'a>(
    text: &'a str,
    labels: &[&str],
) -> Option<&'a str> {
    for label in labels {
        let section_needle = format!("\n\n{label}\n");
        if let Some((_, body)) = text.split_once(&section_needle) {
            return Some(body);
        }
        let prefix_needle = format!("{label}\n");
        if let Some(body) = text.strip_prefix(&prefix_needle) {
            return Some(body);
        }
    }
    None
}

pub(super) fn runtime_smart_context_progressive_critical_exact_ranges(
    artifact_id: &str,
    original: &str,
    line_index: Option<&RuntimeSmartContextArtifactLineIndex>,
    summary: &str,
) -> Option<String> {
    let mut exact_ranges = Vec::new();
    if let Some(line_index) = line_index {
        for range in &line_index.critical_ranges {
            if !runtime_smart_context_artifact_line_index_range_valid(range)
                || range.text.trim().is_empty()
            {
                continue;
            }
            exact_ranges.push(RuntimeSmartContextExactAppendixRange {
                reference: runtime_smart_context_artifact_line_ref(
                    artifact_id,
                    range.start,
                    range.end,
                ),
                body: range.text.clone(),
            });
        }
    }

    if exact_ranges.is_empty() {
        let compacted = summary.to_string();
        let repaired = runtime_smart_context_append_missing_critical_ranges(
            original,
            compacted,
            SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
        );
        if let Some(exact_ranges) = runtime_smart_context_labeled_section_body(
            &repaired,
            &[
                SMART_CONTEXT_LABEL_CRITICAL_EXACT,
                SMART_CONTEXT_LABEL_CRITICAL_EXACT_V1,
                SMART_CONTEXT_LABEL_CRITICAL_EXACT_LEGACY,
            ],
        ) {
            return Some(format!(
                "{SMART_CONTEXT_LABEL_CRITICAL_EXACT}\n{}",
                exact_ranges.trim_end()
            ));
        }
    }

    runtime_smart_context_render_scored_exact_appendix(
        SMART_CONTEXT_LABEL_CRITICAL_EXACT,
        exact_ranges,
        SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
        runtime_smart_context_critical_exact_appendix_score,
    )
    .map(|(appendix, _)| appendix)
}

pub(super) fn runtime_smart_context_critical_exact_appendix_score(
    range: &RuntimeSmartContextExactAppendixRange,
) -> usize {
    prodex_context::count_critical_signals(&range.body)
        .total()
        .saturating_mul(100)
        .saturating_add(10_000usize.saturating_sub(range.body.len().min(10_000)) / 100)
}

pub(super) fn runtime_smart_context_append_missing_critical_ranges(
    original: &str,
    compacted: String,
    max_ranges: usize,
) -> String {
    if !prodex_context::critical_signal_self_check(original, &compacted).has_loss() {
        return compacted;
    }

    let ranges = prodex_context::critical_signal_lost_line_ranges_with_options(
        original,
        &compacted,
        prodex_context::CriticalSignalLineRangeOptions {
            context_lines: 1,
            max_ranges,
            max_range_lines: 6,
        },
    );
    if ranges.is_empty() {
        return compacted;
    }

    let lines = original.lines().collect::<Vec<_>>();
    let mut exact_ranges = Vec::new();
    for range in ranges {
        if range.start == 0 || range.start > lines.len() {
            continue;
        }
        let end = range.end.min(lines.len());
        exact_ranges.push(RuntimeSmartContextExactAppendixRange {
            reference: format!("L{}-L{}:", range.start, end),
            body: lines[range.start - 1..end].join("\n"),
        });
    }
    let Some((appendix, _)) = runtime_smart_context_render_exact_appendix(
        SMART_CONTEXT_LABEL_CRITICAL_EXACT,
        exact_ranges,
    ) else {
        return compacted;
    };

    format!("{compacted}\n\n{appendix}")
}
