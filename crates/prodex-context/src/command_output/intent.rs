use super::*;

pub(super) fn normalize_intent_terms_with_prompt_expansion(terms: &[String]) -> Vec<String> {
    let mut expanded = Vec::<String>::new();
    for term in terms {
        if !term.trim().is_empty() && !term.chars().any(char::is_whitespace) {
            expanded.push(term.clone());
        }
        for extracted in extract_intent_terms_from_prompt(term) {
            expanded.push(extracted);
        }
        if expanded.len() >= MAX_EXTRACTED_INTENT_TERMS.saturating_mul(2) {
            break;
        }
    }
    normalize_intent_terms(&expanded)
        .into_iter()
        .take(MAX_EXTRACTED_INTENT_TERMS)
        .collect()
}

pub fn extract_intent_terms_from_prompt(prompt: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = BTreeMap::<String, ()>::new();
    let mut token = String::new();
    let mut chars = prompt.chars().peekable();

    while let Some(ch) = chars.next() {
        if matches!(ch, '\'' | '"' | '`') {
            push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
            token.clear();

            let quote = ch;
            let mut quoted = String::new();
            for next in chars.by_ref() {
                if next == quote {
                    break;
                }
                quoted.push(next);
            }
            push_prompt_intent_candidate(&mut terms, &mut seen, &quoted, true);
        } else if is_prompt_intent_token_char(ch) {
            token.push(ch);
        } else {
            push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
            token.clear();
        }

        if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
            return terms;
        }
    }
    push_prompt_intent_candidate(&mut terms, &mut seen, &token, false);
    terms
}

#[derive(Debug, Clone)]
struct IntentMatchLine {
    line_number: usize,
    text: String,
    score: usize,
}

fn normalize_intent_terms(terms: &[String]) -> Vec<String> {
    let mut seen = BTreeMap::<String, ()>::new();
    let mut normalized = Vec::new();
    for term in terms {
        let term = normalize_intent_match_text(term.trim());
        if term.is_empty() || seen.contains_key(&term) {
            continue;
        }
        seen.insert(term.clone(), ());
        normalized.push(term);
    }
    normalized
}

fn push_prompt_intent_candidate(
    terms: &mut Vec<String>,
    seen: &mut BTreeMap<String, ()>,
    candidate: &str,
    quoted: bool,
) {
    if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
        return;
    }
    if quoted && candidate.chars().any(char::is_whitespace) {
        for part in candidate.split_whitespace() {
            push_prompt_intent_candidate(terms, seen, part, false);
            if terms.len() >= MAX_EXTRACTED_INTENT_TERMS {
                break;
            }
        }
        return;
    }

    let Some(candidate) = normalize_prompt_intent_candidate(candidate) else {
        return;
    };

    let term = if let Some(code) = prompt_intent_error_code(&candidate) {
        Some(code)
    } else if let Some(path) = prompt_intent_path(&candidate) {
        Some(path)
    } else if looks_like_prompt_intent_file_name(&candidate)
        || looks_like_prompt_intent_symbol(&candidate)
        || quoted && looks_like_quoted_prompt_intent_identifier(&candidate)
    {
        Some(candidate)
    } else {
        None
    };

    let Some(term) = term else {
        return;
    };
    if term.chars().count() > 160 {
        return;
    }

    let key = normalize_intent_match_text(&term);
    if key.is_empty() || seen.contains_key(&key) {
        return;
    }
    seen.insert(key, ());
    terms.push(term);
}

fn normalize_prompt_intent_candidate(candidate: &str) -> Option<String> {
    let candidate = candidate.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\''
                | '`'
                | ','
                | ';'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
                | '!'
                | '?'
                | '*'
                | '#'
                | '='
                | ':'
        )
    });
    let candidate = candidate.trim_end_matches(['.', ',']);
    if candidate.is_empty() || candidate.chars().any(char::is_whitespace) {
        return None;
    }

    Some(candidate.replace('\\', "/"))
}

fn is_prompt_intent_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '\\' | ':' | '$' | '[' | ']')
}

fn prompt_intent_error_code(candidate: &str) -> Option<String> {
    let chars = candidate.chars().collect::<Vec<_>>();
    for index in 0..chars.len() {
        if chars[index].eq_ignore_ascii_case(&'e')
            && index + 4 < chars.len()
            && chars[index + 1..=index + 4]
                .iter()
                .all(|ch| ch.is_ascii_digit())
            && prompt_intent_code_boundary(&chars, index.checked_sub(1))
            && prompt_intent_code_boundary(&chars, Some(index + 5))
        {
            return Some(
                chars[index..=index + 4]
                    .iter()
                    .collect::<String>()
                    .to_ascii_uppercase(),
            );
        }

        if chars[index].eq_ignore_ascii_case(&'t')
            && index + 5 < chars.len()
            && chars[index + 1].eq_ignore_ascii_case(&'s')
            && chars[index + 2..=index + 5]
                .iter()
                .all(|ch| ch.is_ascii_digit())
            && prompt_intent_code_boundary(&chars, index.checked_sub(1))
            && prompt_intent_code_boundary(&chars, Some(index + 6))
        {
            return Some(
                chars[index..=index + 5]
                    .iter()
                    .collect::<String>()
                    .to_ascii_uppercase(),
            );
        }
    }
    None
}

fn prompt_intent_code_boundary(chars: &[char], index: Option<usize>) -> bool {
    index.is_none_or(|index| {
        chars
            .get(index)
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && *ch != '_')
    })
}

fn prompt_intent_path(candidate: &str) -> Option<String> {
    if candidate.contains("://") || candidate.contains(' ') {
        return None;
    }

    let stripped = context_noise_strip_path_location_suffix_supplement(candidate)
        .trim_start_matches("a/")
        .trim_start_matches("b/")
        .trim_start_matches("./")
        .trim_matches('/');
    if stripped.len() < 3 || !stripped.contains('/') || !looks_like_location_path(stripped) {
        return None;
    }

    Some(stripped.to_string())
}

fn looks_like_prompt_intent_file_name(candidate: &str) -> bool {
    if candidate.contains('/') || candidate.contains("://") || candidate.starts_with('-') {
        return false;
    }
    let Some((stem, ext)) = candidate.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && stem.chars().any(|ch| ch.is_ascii_alphanumeric())
        && !ext.is_empty()
        && ext.len() <= 12
        && ext
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn looks_like_prompt_intent_symbol(candidate: &str) -> bool {
    if candidate.contains("://") || candidate.len() < 3 {
        return false;
    }

    if candidate.contains("::") {
        return candidate
            .split("::")
            .all(is_prompt_intent_identifier_segment);
    }

    if candidate.contains('.') && !looks_like_prompt_intent_file_name(candidate) {
        let segments = candidate.split('.').collect::<Vec<_>>();
        return segments.len() >= 2
            && segments
                .iter()
                .all(|segment| is_prompt_intent_identifier_segment(segment));
    }

    if !is_prompt_intent_identifier_segment(candidate) {
        return false;
    }
    let lower = candidate.to_ascii_lowercase();
    if is_noisy_prompt_intent_word(&lower) {
        return false;
    }
    candidate.starts_with("test_")
        || candidate.ends_with("_test")
        || candidate.contains('_')
        || candidate
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch == '_' || ch.is_ascii_digit())
        || has_prompt_intent_camel_shape(candidate)
}

fn looks_like_quoted_prompt_intent_identifier(candidate: &str) -> bool {
    if !is_prompt_intent_identifier_segment(candidate) {
        return false;
    }
    !is_noisy_prompt_intent_word(&candidate.to_ascii_lowercase())
}

fn is_prompt_intent_identifier_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
}

fn has_prompt_intent_camel_shape(candidate: &str) -> bool {
    let uppercase = candidate
        .chars()
        .filter(|ch| ch.is_ascii_uppercase())
        .count();
    let lowercase = candidate.chars().any(|ch| ch.is_ascii_lowercase());
    if uppercase == 0 || !lowercase {
        return false;
    }

    let mut previous_lowercase = false;
    for ch in candidate.chars() {
        if previous_lowercase && ch.is_ascii_uppercase() {
            return true;
        }
        previous_lowercase = ch.is_ascii_lowercase();
    }
    uppercase >= 2
}

fn is_noisy_prompt_intent_word(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "the"
            | "this"
            | "to"
            | "with"
            | "without"
            | "should"
            | "when"
            | "please"
            | "fix"
            | "add"
            | "update"
            | "change"
            | "make"
            | "use"
            | "using"
            | "run"
            | "check"
            | "test"
            | "tests"
            | "error"
            | "failed"
            | "failure"
            | "warning"
            | "output"
            | "command"
            | "context"
            | "smart"
            | "tool"
            | "compaction"
            | "prompt"
            | "request"
            | "ignore"
            | "common"
            | "word"
            | "words"
            | "file"
            | "files"
            | "repo"
            | "path"
            | "paths"
            | "rust"
            | "python"
            | "javascript"
            | "typescript"
    )
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
