use super::*;
use std::collections::BTreeSet;

#[path = "rehydration/appendix.rs"]
mod appendix;
pub(super) use appendix::{
    runtime_smart_context_compact_line_refs_if_shorter,
    runtime_smart_context_consume_rehydrate_budget, runtime_smart_context_line_excerpt,
    runtime_smart_context_render_budgeted_scored_exact_appendix,
    runtime_smart_context_render_exact_appendix,
    runtime_smart_context_render_scored_exact_appendix,
};

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
