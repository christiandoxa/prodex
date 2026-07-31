use super::*;

pub(super) fn compact_diagnostic_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }

    let DiagnosticOutputDetails {
        summary,
        noise_counts,
        key_lines,
        blocks,
        omitted_blocks,
        ..
    } = DiagnosticOutputDetails::collect(&lines, options);

    if summary.is_empty() && noise_counts.is_empty() && key_lines.is_empty() && blocks.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "sum: diag errors={}, failed_tests={}, locations={}, stack={}, exit_statuses={}, noisy={}",
        summary.errors,
        summary.failed_tests.len(),
        summary.locations.len(),
        summary.stack_markers,
        summary.exit_statuses.len(),
        noise_counts.values().sum::<usize>(),
    ));
    push_labeled_lines(
        &mut output,
        "root causes",
        &summary.root_causes,
        options.max_lines.max(24).saturating_div(6).max(3),
    );
    push_labeled_lines(
        &mut output,
        "diagnostics",
        &summary.diagnostic_headers,
        options.max_lines.max(24).saturating_div(4).max(4),
    );
    push_labeled_lines(
        &mut output,
        "locations",
        &summary.locations,
        options.max_lines.max(24).saturating_div(5).max(4),
    );
    push_labeled_lines(
        &mut output,
        "failed tests",
        &summary.failed_tests,
        options.max_lines.max(24).saturating_div(5).max(4),
    );
    push_labeled_lines(
        &mut output,
        "exit statuses",
        &summary.exit_statuses,
        options.max_lines.max(24).saturating_div(6).max(3),
    );
    push_labeled_lines(
        &mut output,
        "key lines",
        &key_lines,
        options.max_lines.max(24).saturating_div(6).max(3),
    );

    if !blocks.is_empty() {
        output.push("critical blocks:".to_string());
        for block in blocks {
            output.push(format!("-- {} --", block.label));
            for line in block.lines {
                output.push(truncate_command_line(&line, options.max_line_chars));
            }
        }
    }
    if omitted_blocks > 0 {
        output.push(format!(
            "[... omitted {omitted_blocks} additional critical blocks ...]"
        ));
    }
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }

    finalize_compacted_command_output(CommandOutputKind::Diagnostics, input, output, options)
}

#[derive(Default)]
struct DiagnosticOutputDetails {
    summary: CommandDiagnosticSummary,
    noise_counts: BTreeMap<String, usize>,
    key_lines: Vec<String>,
    blocks: Vec<CommandCriticalBlock>,
    used_block_lines: usize,
    omitted_blocks: usize,
}

impl DiagnosticOutputDetails {
    fn collect(lines: &[&str], options: &CommandOutputCompactOptions) -> Self {
        let mut details = Self::default();
        let block_limit = diagnostic_block_line_limit(options);
        let block_budget = options.max_lines.max(24).saturating_div(3).max(6);
        let mut index = 0usize;
        while index < lines.len() {
            index = details.record_line(
                lines,
                index,
                block_limit,
                block_budget,
                options.max_line_chars,
            );
        }
        details
    }

    fn record_line(
        &mut self,
        lines: &[&str],
        index: usize,
        block_limit: usize,
        block_budget: usize,
        max_line_chars: usize,
    ) -> usize {
        let line = lines[index];
        if let Some(label) = diagnostic_noise_label(line) {
            *self.noise_counts.entry(label.to_string()).or_default() += 1;
            if is_diagnostic_key_line(line) {
                push_unique_truncated_line(&mut self.key_lines, line, max_line_chars);
            }
            return index + 1;
        }

        if is_diagnostic_block_start(line) {
            self.summary.record_line(line);
            let (block, next_index) = collect_diagnostic_block(lines, index, block_limit);
            self.summary.record_block_signals(&block);
            self.record_block(block, block_budget);
            return next_index;
        }

        if is_critical_preserve_line(line) || count_file_location_signals(line) > 0 {
            self.summary.record_line(line);
            push_unique_truncated_line(&mut self.key_lines, line, max_line_chars);
            return index + 1;
        }

        if is_success_output_failure_signal_line(line)
            || is_success_output_warning_signal_line(line)
        {
            push_unique_truncated_line(&mut self.key_lines, line, max_line_chars);
        }
        index + 1
    }

    fn record_block(&mut self, block: CommandCriticalBlock, block_budget: usize) {
        if self.used_block_lines.saturating_add(block.lines.len()) <= block_budget
            || self.blocks.is_empty()
        {
            self.used_block_lines = self.used_block_lines.saturating_add(block.lines.len());
            self.blocks.push(block);
        } else {
            self.omitted_blocks += 1;
        }
    }
}
