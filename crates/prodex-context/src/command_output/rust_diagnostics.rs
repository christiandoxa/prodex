use super::*;

pub(super) fn compact_rust_diagnostic_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }

    let mut summary = RustDiagnosticSummary::default();
    let mut noise_counts = BTreeMap::<String, usize>::new();
    let mut key_lines = Vec::<String>::new();
    let mut blocks = Vec::<RustCriticalBlock>::new();
    let block_limit = rust_block_line_limit(options);
    let block_budget = options.max_lines.max(24).saturating_div(3).max(4);
    let mut used_block_lines = 0usize;
    let mut omitted_blocks = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index];
        if let Some(label) = rust_noise_label(line) {
            *noise_counts.entry(label.to_string()).or_default() += 1;
            if is_rust_success_summary_line(line) {
                push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            }
            index += 1;
            continue;
        }

        if let Some(severity) = rust_diagnostic_severity(line) {
            summary.record_diagnostic(severity, line);
            let (block, next_index) = collect_rust_diagnostic_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if let Some(test_name) = rust_failed_test_name(line) {
            summary.record_failed_test(test_name);
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            let (block, next_index) = collect_rust_failure_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if let Some(test_name) = rust_failure_separator_name(line) {
            summary.record_failed_test(test_name);
            let (block, next_index) = collect_rust_failure_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if is_rust_panic_line(line) || is_rust_backtrace_start(line) {
            let (block, next_index) = collect_rust_failure_block(&lines, index, block_limit);
            summary.record_block_signals(&block);
            if used_block_lines.saturating_add(block.lines.len()) <= block_budget
                || blocks.is_empty()
            {
                used_block_lines = used_block_lines.saturating_add(block.lines.len());
                blocks.push(block);
            } else {
                omitted_blocks += 1;
            }
            index = next_index;
            continue;
        }

        if is_rust_exit_status_line(line) {
            summary.record_exit_status(line);
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            index += 1;
            continue;
        }

        if is_rust_location_line(line) {
            summary.record_location(line);
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            index += 1;
            continue;
        }

        if is_rust_failure_summary_line(line) {
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            index += 1;
            continue;
        }

        index += 1;
    }

    if summary.is_empty() && noise_counts.is_empty() && key_lines.is_empty() && blocks.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "sum: rust errors={}, warnings={}, failed_tests={}, panics={}, exit_statuses={}, noisy={}",
        summary.errors,
        summary.warnings,
        summary.failed_tests.len(),
        summary.panics,
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

    finalize_compacted_command_output(CommandOutputKind::RustDiagnostics, input, output, options)
}
