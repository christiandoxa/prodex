use super::dedupe::runtime_smart_context_dedupe_progressive_summary_chunks;
use super::*;

pub(super) fn runtime_smart_context_progressive_tool_output_summary(
    artifact: &runtime_proxy_crate::SmartContextArtifactRef,
    text: &str,
    indexes: RuntimeSmartContextArtifactIndexes<'_>,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    metadata: &RuntimeSmartContextToolOutputCompactionMetadata,
    intent_terms: &[String],
) -> String {
    let compacted = runtime_smart_context_compact_successful_tool_output(
        text,
        tier,
        preview_byte_limit,
        metadata,
    )
    .unwrap_or_else(|| {
        runtime_smart_context_compact_tool_output_preserving_critical(
            text,
            tier,
            preview_byte_limit,
            metadata.kind_hint,
            intent_terms,
        )
    });
    let summary = runtime_smart_context_progressive_summary_excerpt(&compacted);
    let summary = runtime_smart_context_dedupe_progressive_summary_chunks(
        &artifact.id,
        text,
        &summary,
        indexes.chunk_index,
    );
    let mut sections = Vec::new();
    if !summary.trim().is_empty() {
        sections.push(format!(
            "{SMART_CONTEXT_LABEL_SUMMARY}\n{}",
            summary.trim_end()
        ));
    }
    if let Some(critical_ranges) = runtime_smart_context_progressive_critical_exact_ranges(
        &artifact.id,
        text,
        indexes.line_index,
        &summary,
    ) {
        sections.push(critical_ranges);
    }
    if sections.is_empty() {
        sections.push(format!(
            "{SMART_CONTEXT_LABEL_SUMMARY}\nlarge output omitted; use ref/lines"
        ));
    }
    sections.join("\n\n")
}

fn runtime_smart_context_progressive_summary_excerpt(text: &str) -> String {
    if text.len() <= SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES {
        return text.to_string();
    }

    let mut summary = String::new();
    for line in text.lines() {
        let next_len = summary
            .len()
            .saturating_add((!summary.is_empty()) as usize)
            .saturating_add(line.len());
        if next_len > SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES {
            break;
        }
        if !summary.is_empty() {
            summary.push('\n');
        }
        summary.push_str(line);
    }
    if summary.is_empty() {
        summary.extend(
            text.chars()
                .take(SMART_CONTEXT_TOOL_PROGRESSIVE_SUMMARY_MAX_BYTES),
        );
    }
    summary.push_str("\n[trunc; use ref for full]");
    summary
}

fn runtime_smart_context_compact_successful_tool_output(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    metadata: &RuntimeSmartContextToolOutputCompactionMetadata,
) -> Option<String> {
    let max_lines =
        runtime_smart_context_tool_preview_max_lines(tier, preview_byte_limit).unwrap_or(40);
    let report = prodex_context::compact_successful_command_output_with_options(
        text,
        &prodex_context::CommandSuccessOutputCompactOptions {
            command: metadata.command.clone(),
            exit_code: metadata.exit_code,
            min_lines_to_compact: max_lines.saturating_mul(2).max(20),
            max_touched_files: max_lines.clamp(8, 48),
            max_key_lines: (max_lines / 2).clamp(4, 24),
            max_line_chars: SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS,
        },
    );
    if report.compacted && !report.failure_suspected && report.output.len() < text.len() {
        Some(report.output)
    } else {
        None
    }
}

fn runtime_smart_context_compact_tool_output_preserving_critical(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    command_kind_hint: Option<prodex_context::CommandOutputKind>,
    intent_terms: &[String],
) -> String {
    let compacted = runtime_smart_context_compact_tool_output(
        text,
        tier,
        preview_byte_limit,
        command_kind_hint,
        intent_terms,
    );
    runtime_smart_context_append_missing_critical_ranges(
        text,
        compacted,
        SMART_CONTEXT_SURGICAL_CRITICAL_MAX_RANGES,
    )
}

fn runtime_smart_context_compact_tool_output(
    text: &str,
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
    command_kind_hint: Option<prodex_context::CommandOutputKind>,
    intent_terms: &[String],
) -> String {
    let Some(max_lines) = runtime_smart_context_tool_preview_max_lines(tier, preview_byte_limit)
    else {
        return text.to_string();
    };
    let options = prodex_context::CommandOutputCompactOptions {
        max_lines,
        head_lines: max_lines.saturating_mul(2) / 3,
        tail_lines: max_lines / 3,
        max_line_chars: SMART_CONTEXT_TOOL_PREVIEW_MAX_LINE_CHARS,
        ..prodex_context::CommandOutputCompactOptions::default()
    };
    prodex_context::compact_command_output_with_intent_options(
        text,
        &prodex_context::CommandOutputIntentCompactOptions::new(options, intent_terms.to_vec())
            .with_kind_hint(command_kind_hint),
    )
    .output
}

pub(in crate::runtime_proxy::smart_context) fn runtime_smart_context_tool_preview_max_lines(
    tier: runtime_proxy_crate::SmartContextTokenBudgetTier,
    preview_byte_limit: usize,
) -> Option<usize> {
    if tier == runtime_proxy_crate::SmartContextTokenBudgetTier::Exact
        && preview_byte_limit == usize::MAX
    {
        return None;
    }

    let budget_lines = preview_byte_limit / SMART_CONTEXT_TOOL_PREVIEW_ESTIMATED_LINE_BYTES;
    let (floor, cap) = match tier {
        runtime_proxy_crate::SmartContextTokenBudgetTier::Minimal => (8, 24),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Condensed => (24, 80),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Large => (80, 240),
        runtime_proxy_crate::SmartContextTokenBudgetTier::Exact => (120, 360),
    };
    Some(budget_lines.max(floor).min(cap))
}
