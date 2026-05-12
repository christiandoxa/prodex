use super::*;

mod prompt;
pub use prompt::extract_intent_terms_from_prompt;
pub(crate) use prompt::normalize_intent_terms_with_prompt_expansion;

#[derive(Debug, Clone)]
struct IntentMatchLine {
    line_number: usize,
    text: String,
    score: usize,
}

pub(super) fn compact_command_output_for_intent(
    original: &str,
    base_output: &str,
    kind: CommandOutputKind,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> String {
    if !command_output_kind_supports_intent_compaction(kind) {
        return base_output.to_string();
    }
    if kind == CommandOutputKind::Search {
        return compact_search_output_for_intent(original, base_output, options, intent_terms);
    }
    if kind == CommandOutputKind::FileList {
        return compact_file_list_output_for_intent(original, base_output, options, intent_terms);
    }
    if kind == CommandOutputKind::GitDiff {
        return compact_git_diff_output_with_intent(original, options, intent_terms);
    }

    let intent_matches = collect_intent_matching_lines(original, intent_terms, options);
    if intent_matches.is_empty() {
        return base_output.to_string();
    }

    let base_lines = command_lines(base_output);
    let max_lines = options.max_lines.saturating_add(1).max(6);
    let mut output = Vec::new();
    if let Some(header) = base_lines.first() {
        output.push((*header).to_string());
    } else {
        output.push(format!(
            "pcs: {} ({}->intent)",
            kind.label(),
            count_text_lines(original),
        ));
    }

    output.push(format!(
        "int: {} lines for {}",
        intent_matches.len(),
        truncate_command_line(&intent_terms.join(", "), options.max_line_chars),
    ));

    let reserved_for_baseline = 3usize.min(max_lines.saturating_sub(output.len()));
    let mut intent_budget = max_lines
        .saturating_sub(output.len())
        .saturating_sub(reserved_for_baseline)
        .max(1);
    intent_budget = intent_budget.min(max_lines.saturating_div(2).max(2));
    for intent_match in intent_matches.iter().take(intent_budget) {
        output.push(format!(
            "  L{}: {}",
            intent_match.line_number,
            truncate_command_line(&intent_match.text, options.max_line_chars),
        ));
    }
    if intent_matches.len() > intent_budget {
        output.push(format!(
            "  [... {} more intent-matching lines ...]",
            intent_matches.len() - intent_budget
        ));
    }

    let remaining = max_lines.saturating_sub(output.len());
    if remaining >= 2 && base_lines.len() > 1 {
        output.push("base:".to_string());
        let baseline_budget = max_lines.saturating_sub(output.len()).max(1);
        let baseline = base_lines
            .iter()
            .skip(1)
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>();
        push_head_tail_lines(
            &mut output,
            &baseline,
            baseline_budget,
            options.max_line_chars,
            "baseline lines",
            "  ",
        );
    }

    lines_to_text(output)
}

fn compact_search_output_for_intent(
    original: &str,
    base_output: &str,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> String {
    let (files, _) = collect_search_output_matches(original);
    let total_matches = files.values().map(Vec::len).sum::<usize>();
    if total_matches == 0 {
        return base_output.to_string();
    }

    let mut relevant = Vec::<(usize, String, Vec<SearchMatch>)>::new();
    let mut other_files = 0usize;
    let mut other_matches = 0usize;
    for (path, matches) in files {
        let path_score = score_intent_text(&path, intent_terms).saturating_mul(4);
        let mut scored = matches
            .iter()
            .cloned()
            .map(|search_match| {
                let score =
                    path_score.saturating_add(score_intent_text(&search_match.text, intent_terms));
                (score, search_match)
            })
            .collect::<Vec<_>>();
        let file_score = scored.iter().map(|(score, _)| *score).max().unwrap_or(0);
        if file_score == 0 {
            other_files += 1;
            other_matches += matches.len();
            continue;
        }
        scored.sort_by_key(|(score, search_match)| {
            (
                Reverse(*score),
                search_match.line_number.unwrap_or(usize::MAX),
                search_match.text.clone(),
            )
        });
        relevant.push((
            file_score,
            path,
            scored
                .into_iter()
                .map(|(_, search_match)| search_match)
                .collect(),
        ));
    }

    if relevant.is_empty() {
        return base_output.to_string();
    }

    relevant.sort_by_key(|(score, path, _)| (Reverse(*score), path.clone()));
    let base_lines = command_lines(base_output);
    let max_lines = options.max_lines.saturating_add(1).max(8);
    let mut output = Vec::<String>::new();
    push_intent_header(
        &mut output,
        &base_lines,
        CommandOutputKind::Search,
        original,
    );
    let relevant_matches = relevant
        .iter()
        .map(|(_, _, matches)| matches.len())
        .sum::<usize>();
    output.push(format!(
        "int: {} search matches across {} files for {}",
        relevant_matches,
        relevant.len(),
        truncate_command_line(&intent_terms.join(", "), options.max_line_chars),
    ));
    output.push(format!(
        "overflow: {} other matches across {} files",
        other_matches, other_files,
    ));
    output.push("rel search:".to_string());

    let mut budget = max_lines.saturating_sub(output.len()).max(1);
    let reserve = usize::from(other_matches > 0).saturating_add(usize::from(base_lines.len() > 1));
    budget = budget.saturating_sub(reserve).max(1);
    let per_file = options.max_search_matches_per_file.max(1);
    let mut hidden_relevant = 0usize;
    for (_, path, matches) in &relevant {
        if budget <= 1 {
            hidden_relevant += matches.len();
            continue;
        }
        output.push(format!("{path} ({} relevant matches):", matches.len()));
        budget = budget.saturating_sub(1);
        let shown = matches.len().min(per_file).min(budget);
        for search_match in matches.iter().take(shown) {
            let prefix = search_match
                .line_number
                .map(|line| format!("{line}: "))
                .unwrap_or_default();
            output.push(format!(
                "  {}{}",
                prefix,
                truncate_command_line(&search_match.text, options.max_line_chars),
            ));
        }
        budget = budget.saturating_sub(shown);
        if matches.len() > shown {
            output.push(format!(
                "  [... {} more relevant matches in this file ...]",
                matches.len() - shown
            ));
            budget = budget.saturating_sub(1);
        }
    }
    if hidden_relevant > 0 {
        output.push(format!(
            "[... omitted {hidden_relevant} additional relevant search matches ...]"
        ));
    }
    push_intent_baseline_tail(&mut output, &base_lines, max_lines, options);
    lines_to_text(output)
}

fn compact_file_list_output_for_intent(
    original: &str,
    base_output: &str,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> String {
    let entries = collect_file_list_entries(original);
    if entries.is_empty() {
        return base_output.to_string();
    }

    let mut relevant = Vec::<(usize, String)>::new();
    let mut overflow = Vec::<String>::new();
    for entry in entries {
        let score = score_intent_text(&entry, intent_terms);
        if score == 0 {
            overflow.push(entry);
        } else {
            relevant.push((score, entry));
        }
    }
    if relevant.is_empty() {
        return base_output.to_string();
    }

    relevant.sort_by_key(|(score, path)| (Reverse(*score), path.clone()));
    let base_lines = command_lines(base_output);
    let max_lines = options.max_lines.saturating_add(1).max(8);
    let mut output = Vec::<String>::new();
    push_intent_header(
        &mut output,
        &base_lines,
        CommandOutputKind::FileList,
        original,
    );
    output.push(format!(
        "int: {} paths for {}",
        relevant.len(),
        truncate_command_line(&intent_terms.join(", "), options.max_line_chars),
    ));
    if !overflow.is_empty() {
        let roots = count_success_output_path_roots(&overflow);
        let extensions = count_success_output_path_extensions(&overflow);
        output.push(format!("overflow: {} other file entries", overflow.len()));
        output.push(format_count_map("overflow roots", &roots, 6));
        output.push(format_count_map("overflow extensions", &extensions, 6));
    }
    output.push("rel paths:".to_string());

    let mut budget = max_lines.saturating_sub(output.len()).max(1);
    let reserve = usize::from(base_lines.len() > 1);
    budget = budget.saturating_sub(reserve).max(1);
    let path_limit = options.max_path_entries.max(1).min(budget);
    for (_, path) in relevant.iter().take(path_limit) {
        output.push(format!(
            "  {}",
            truncate_command_line(path, options.max_line_chars)
        ));
    }
    if relevant.len() > path_limit {
        output.push(format!(
            "  [... {} more relevant paths ...]",
            relevant.len() - path_limit
        ));
    }
    push_intent_baseline_tail(&mut output, &base_lines, max_lines, options);
    lines_to_text(output)
}

fn push_intent_header(
    output: &mut Vec<String>,
    base_lines: &[&str],
    kind: CommandOutputKind,
    original: &str,
) {
    if let Some(header) = base_lines.first() {
        output.push((*header).to_string());
    } else {
        output.push(format!(
            "pcs: {} ({}->intent)",
            kind.label(),
            count_text_lines(original),
        ));
    }
}

fn push_intent_baseline_tail(
    output: &mut Vec<String>,
    base_lines: &[&str],
    max_lines: usize,
    options: &CommandOutputCompactOptions,
) {
    let remaining = max_lines.saturating_sub(output.len());
    if remaining < 3 || base_lines.len() <= 1 {
        return;
    }
    output.push("base:".to_string());
    let baseline = base_lines
        .iter()
        .skip(1)
        .map(|line| (*line).to_string())
        .collect::<Vec<_>>();
    push_head_tail_lines(
        output,
        &baseline,
        max_lines.saturating_sub(output.len()).max(1),
        options.max_line_chars,
        "baseline lines",
        "  ",
    );
}

pub(super) fn score_intent_text(value: &str, intent_terms: &[String]) -> usize {
    let value = normalize_intent_match_text(value);
    intent_terms.iter().fold(0usize, |score, term| {
        if term.is_empty() {
            return score;
        }
        let mut term_score = 0usize;
        if value.contains(term.as_str()) {
            term_score = term_score
                .saturating_add(4)
                .saturating_add(term.len().min(48).saturating_div(8));
        }
        if let Some(basename) = intent_term_basename(term)
            && basename != term.as_str()
            && value.contains(basename)
        {
            term_score = term_score.saturating_add(2);
        }
        score.saturating_add(term_score)
    })
}

fn intent_term_basename(term: &str) -> Option<&str> {
    term.rsplit('/').next().filter(|basename| {
        !basename.is_empty() && basename.len() >= 3 && basename.len() < term.len()
    })
}

fn command_output_kind_supports_intent_compaction(kind: CommandOutputKind) -> bool {
    matches!(
        kind,
        CommandOutputKind::GitDiff
            | CommandOutputKind::RustDiagnostics
            | CommandOutputKind::Diagnostics
            | CommandOutputKind::Search
            | CommandOutputKind::FileList
            | CommandOutputKind::Plain
    )
}

fn collect_intent_matching_lines(
    input: &str,
    intent_terms: &[String],
    options: &CommandOutputCompactOptions,
) -> Vec<IntentMatchLine> {
    let lines = command_lines(input);
    let mut selected = BTreeMap::<usize, usize>::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let score = score_intent_text(trimmed, intent_terms);
        if score == 0 {
            continue;
        }
        selected
            .entry(index)
            .and_modify(|existing| *existing = (*existing).max(score))
            .or_insert(score);
        for nearby in intent_context_line_range(index, lines.len()) {
            if nearby == index || lines[nearby].trim().is_empty() {
                continue;
            }
            selected
                .entry(nearby)
                .and_modify(|existing| *existing = (*existing).max(score.saturating_sub(1)))
                .or_insert(score.saturating_sub(1).max(1));
        }
    }

    let mut matches = selected
        .into_iter()
        .map(|(index, score)| IntentMatchLine {
            line_number: index + 1,
            text: truncate_command_line(lines[index].trim(), options.max_line_chars),
            score,
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|line| (Reverse(line.score), line.line_number));
    matches
}

pub(super) fn intent_line_matches(line: &str, intent_terms: &[String]) -> bool {
    score_intent_text(line, intent_terms) > 0
}

fn normalize_intent_match_text(value: &str) -> String {
    value.replace('\\', "/").trim().to_ascii_lowercase()
}

fn intent_context_line_range(index: usize, total: usize) -> std::ops::RangeInclusive<usize> {
    let start = index.saturating_sub(1);
    let end = (index + 1).min(total.saturating_sub(1));
    start..=end
}

pub(super) fn ensure_no_critical_signal_loss_for_intent(
    original: &str,
    candidate: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    if critical_signal_self_check(original, candidate).passed() {
        return candidate.to_string();
    }

    let mut output = command_lines(candidate)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    output.push("critical signal rescue:".to_string());
    for line in command_lines(original) {
        if !is_critical_preserve_line(line) {
            continue;
        }
        output.push(truncate_command_line(line.trim(), options.max_line_chars));
        let text = lines_to_text(output.clone());
        if critical_signal_self_check(original, &text).passed() {
            return text;
        }
    }

    let text = lines_to_text(output);
    if critical_signal_self_check(original, &text).passed() {
        text
    } else {
        candidate.to_string()
    }
}
