use super::*;

pub(super) fn compact_noisy_success_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    let lines = command_lines(input);
    if lines.is_empty() {
        return String::new();
    }
    if !count_critical_signals(input).is_empty()
        || lines.iter().any(|line| {
            is_success_output_failure_signal_line(line)
                || is_success_output_warning_signal_line(line)
        })
    {
        return smart_truncate_command_output(input, options);
    }

    let mut noise_counts = BTreeMap::<String, usize>::new();
    let mut key_lines = Vec::<String>::new();
    let mut critical_lines = Vec::<String>::new();
    for line in lines {
        if let Some(label) = noisy_success_label(line) {
            *noise_counts.entry(label.to_string()).or_default() += 1;
            if is_noisy_success_key_line(line) {
                push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
            }
            continue;
        }
        if is_noisy_success_key_line(line) {
            push_unique_truncated_line(&mut key_lines, line, options.max_line_chars);
        }
        if is_critical_preserve_line(line) {
            push_unique_truncated_line(&mut critical_lines, line, options.max_line_chars);
        }
    }

    if noise_counts.is_empty() && key_lines.is_empty() && critical_lines.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "sum: success noisy={}, critical={}",
        noise_counts.values().sum::<usize>(),
        critical_lines.len(),
    ));
    if !noise_counts.is_empty() {
        output.push(format_count_map("noise", &noise_counts, 10));
    }
    push_labeled_lines(
        &mut output,
        "key lines",
        &key_lines,
        options.max_lines.max(24).saturating_div(3).max(4),
    );
    push_labeled_lines(
        &mut output,
        "critical lines",
        &critical_lines,
        options.max_lines.max(24).saturating_div(4).max(4),
    );

    finalize_compacted_command_output(CommandOutputKind::NoisySuccess, input, output, options)
}
