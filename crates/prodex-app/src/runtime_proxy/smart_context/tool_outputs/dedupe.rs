use super::*;

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_dedupe_progressive_summary_chunks(
    artifact_id: &str,
    original: &str,
    summary: &str,
    chunk_index: Option<&RuntimeSmartContextArtifactChunkIndex>,
) -> String {
    let Some(chunk_index) = chunk_index.filter(|index| index.complete) else {
        return summary.to_string();
    };
    if chunk_index.duplicate_chunks.is_empty() {
        return summary.to_string();
    }
    let lines = original.lines().collect::<Vec<_>>();
    let plans = chunk_index
        .duplicate_chunks
        .iter()
        .filter_map(|duplicate| {
            runtime_smart_context_duplicate_chunk_summary_plan(artifact_id, &lines, duplicate)
        })
        .collect::<Vec<_>>();
    if plans.is_empty() {
        return summary.to_string();
    }

    let mut candidate = summary.to_string();
    let mut entries = Vec::new();
    for plan in plans {
        let matches = candidate.match_indices(&plan.text).count();
        if matches < 2 {
            continue;
        }
        let marker = format!("[psc dup h={} b={}]", plan.content_hash, plan.byte_len);
        let entry = format!(
            "- h={} b={} x={} refs={}",
            plan.content_hash,
            plan.byte_len,
            plan.occurrence_count,
            runtime_smart_context_compact_line_refs_if_shorter(&plan.refs)
        );
        let removed_bytes = plan.text.len().saturating_mul(matches.saturating_sub(1));
        let added_bytes = marker
            .len()
            .saturating_mul(matches.saturating_sub(1))
            .saturating_add(entry.len())
            .saturating_add(1);
        if added_bytes >= removed_bytes {
            continue;
        }
        candidate = runtime_smart_context_replace_repeated_exact_text_after_first(
            &candidate, &plan.text, &marker,
        );
        entries.push(entry);
    }

    if entries.is_empty() {
        return summary.to_string();
    }
    candidate.push_str("\n\n");
    candidate.push_str(SMART_CONTEXT_LABEL_DUPLICATE_CHUNKS);
    candidate.push('\n');
    candidate.push_str(&entries.join("\n"));
    if candidate.len() < summary.len() {
        candidate
    } else {
        summary.to_string()
    }
}

fn runtime_smart_context_duplicate_chunk_summary_plan(
    artifact_id: &str,
    lines: &[&str],
    duplicate: &RuntimeSmartContextArtifactDuplicateChunkFingerprint,
) -> Option<RuntimeSmartContextDuplicateChunkSummaryPlan> {
    if duplicate.occurrence_count < 2
        || duplicate.occurrence_count != duplicate.occurrences.len()
        || duplicate.byte_len == 0
    {
        return None;
    }
    let mut refs = Vec::new();
    let mut ranges = BTreeSet::<(usize, usize)>::new();
    let mut exact_text: Option<String> = None;
    for occurrence in &duplicate.occurrences {
        if occurrence.start == 0
            || occurrence.end < occurrence.start
            || !ranges.insert((occurrence.start, occurrence.end))
        {
            return None;
        }
        let text = runtime_smart_context_line_excerpt(lines, occurrence.start, occurrence.end)?;
        if text.len() != duplicate.byte_len
            || runtime_proxy_crate::smart_context_hash_text(&text) != duplicate.content_hash
        {
            return None;
        }
        if let Some(existing) = exact_text.as_ref() {
            if existing != &text {
                return None;
            }
        } else {
            exact_text = Some(text);
        }
        refs.push(runtime_smart_context_artifact_line_ref(
            artifact_id,
            occurrence.start,
            occurrence.end,
        ));
    }
    Some(RuntimeSmartContextDuplicateChunkSummaryPlan {
        text: exact_text?,
        content_hash: duplicate.content_hash.clone(),
        byte_len: duplicate.byte_len,
        occurrence_count: duplicate.occurrence_count,
        refs,
    })
}

fn runtime_smart_context_replace_repeated_exact_text_after_first(
    text: &str,
    needle: &str,
    replacement: &str,
) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let mut rendered = String::with_capacity(text.len());
    let mut cursor = 0usize;
    let mut seen = 0usize;
    for (offset, _) in text.match_indices(needle) {
        rendered.push_str(&text[cursor..offset]);
        seen = seen.saturating_add(1);
        if seen == 1 {
            rendered.push_str(needle);
        } else {
            rendered.push_str(replacement);
        }
        cursor = offset.saturating_add(needle.len());
    }
    rendered.push_str(&text[cursor..]);
    rendered
}
