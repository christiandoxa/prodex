use super::*;

pub(super) fn compact_git_status_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    let mut summary = GitStatusSummary::default();
    let lines = command_lines(input);
    let short_format = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .all(|line| is_short_git_status_line(line) || line.starts_with("## "));

    if short_format {
        for line in lines {
            parse_short_git_status_line(line, &mut summary);
        }
    } else {
        parse_long_git_status_lines(&lines, &mut summary);
    }

    let category_limit = options.max_path_entries.max(1).div_ceil(6).max(4);
    let mut output = Vec::new();
    output.push("sum: git status".to_string());
    if let Some(branch) = summary.branch {
        output.push(format!("branch: {branch}"));
    }
    if summary.clean {
        output.push("clean: true".to_string());
    }
    push_item_summary(&mut output, "staged", &summary.staged, category_limit);
    push_item_summary(&mut output, "modified", &summary.modified, category_limit);
    push_item_summary(&mut output, "deleted", &summary.deleted, category_limit);
    push_item_summary(&mut output, "renamed", &summary.renamed, category_limit);
    push_item_summary(
        &mut output,
        "conflicted",
        &summary.conflicted,
        category_limit,
    );
    push_item_summary(&mut output, "untracked", &summary.untracked, category_limit);
    push_item_summary(&mut output, "other", &summary.other, category_limit);

    if output.len() == 1 {
        return smart_truncate_command_output(input, options);
    }

    finalize_compacted_command_output(CommandOutputKind::GitStatus, input, output, options)
}

pub(super) fn compact_git_diff_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    compact_git_diff_output_with_intent(input, options, &[])
}

pub(super) fn compact_git_diff_output_with_intent(
    input: &str,
    options: &CommandOutputCompactOptions,
    intent_terms: &[String],
) -> String {
    let lines = command_lines(input);
    let sections = split_git_diff_sections(&lines);
    if sections.is_empty() {
        if let Some(output) = compact_git_diff_stat_output(&lines, options) {
            let output = finalize_compacted_command_output(
                CommandOutputKind::GitDiff,
                input,
                output,
                options,
            );
            return ensure_no_critical_signal_loss_for_intent(input, &output, options);
        }
        return smart_truncate_command_output(input, options);
    }

    let summaries = sections
        .iter()
        .map(|section| summarize_git_diff_section(section))
        .collect::<Vec<_>>();
    let total_added = summaries.iter().map(|summary| summary.added).sum::<usize>();
    let total_removed = summaries
        .iter()
        .map(|summary| summary.removed)
        .sum::<usize>();
    let total_hunks = summaries.iter().map(|summary| summary.hunks).sum::<usize>();

    let intent_focused = !intent_terms.is_empty();
    let structural_line_count = sections
        .iter()
        .map(|section| {
            section
                .iter()
                .filter(|line| is_git_diff_excerpt_structural_line(line, intent_focused))
                .count()
        })
        .sum::<usize>();
    let fixed_body_lines = 1usize
        .saturating_add(usize::from(intent_focused))
        .saturating_add(summaries.len())
        .saturating_add(2)
        .saturating_add(structural_line_count)
        .saturating_add(sections.len());
    let detail_budget = options
        .max_lines
        .saturating_sub(1)
        .saturating_sub(fixed_body_lines);
    let per_file_budget = detail_budget
        .checked_div(sections.len().max(1))
        .unwrap_or(0)
        .max(usize::from(detail_budget > 0));

    let mut output = Vec::new();
    output.push(format!(
        "sum: git diff files={}, +{}, -{}, hunks={}",
        summaries.len(),
        total_added,
        total_removed,
        total_hunks,
    ));
    if !intent_terms.is_empty() {
        output.push(format!(
            "int: git diff focus for {}",
            truncate_command_line(&intent_terms.join(", "), options.max_line_chars)
        ));
    }
    for summary in &summaries {
        let binary = if summary.binary { ", binary" } else { "" };
        let mut line = format!(
            "{}: +{}, -{}, {} hunks{}",
            summary.path, summary.added, summary.removed, summary.hunks, binary,
        );
        if !summary.semantic_contexts.is_empty() {
            line.push_str(&format!(
                ", ctx={}",
                truncate_command_line(&summary.semantic_contexts.join(" | "), 140)
            ));
        }
        output.push(line);
    }
    output.push(String::new());
    output.push("diff excerpts:".to_string());

    for (section, summary) in sections.iter().zip(summaries.iter()) {
        let section_score = score_git_diff_section_for_intent(section, summary, intent_terms);
        if !intent_terms.is_empty() && section_score == 0 {
            output.push(format!(
                "{}: [... omitted unchanged-to-intent diff detail ...]",
                summary.path
            ));
            continue;
        }
        let section_budget = per_file_budget;
        let selected_detail =
            select_git_diff_detail_line_indexes(section, section_budget, intent_terms);
        let mut omitted_detail = 0usize;
        for (line_index, line) in section.iter().enumerate() {
            if is_git_diff_excerpt_structural_line(line, intent_focused)
                || selected_detail.contains_key(&line_index)
            {
                output.push(truncate_command_line(line, options.max_line_chars));
            } else {
                omitted_detail += 1;
            }
        }
        if omitted_detail > 0 {
            output.push(format!(
                "[... omitted {} diff lines for {} ...]",
                omitted_detail, summary.path,
            ));
        }
    }

    let output =
        finalize_compacted_command_output(CommandOutputKind::GitDiff, input, output, options);
    ensure_no_critical_signal_loss_for_intent(input, &output, options)
}

fn score_git_diff_section_for_intent(
    section: &[&str],
    summary: &GitDiffSummary,
    intent_terms: &[String],
) -> usize {
    if intent_terms.is_empty() {
        return 0;
    }
    let mut score = score_intent_text(&summary.path, intent_terms).saturating_mul(6);
    for context in &summary.semantic_contexts {
        score = score.saturating_add(score_intent_text(context, intent_terms).saturating_mul(4));
    }
    for line in section {
        score = score.saturating_add(score_intent_text(line, intent_terms));
    }
    score
}

fn select_git_diff_detail_line_indexes(
    section: &[&str],
    budget: usize,
    intent_terms: &[String],
) -> BTreeMap<usize, ()> {
    if budget == 0 {
        return BTreeMap::new();
    }

    let mut intent_hits = Vec::<usize>::new();
    for (index, line) in section.iter().enumerate() {
        if !is_git_diff_structural_line(line) && intent_line_matches(line, intent_terms) {
            intent_hits.push(index);
        }
    }

    let mut candidates = Vec::<(u8, usize)>::new();
    for (index, line) in section.iter().enumerate() {
        if is_git_diff_structural_line(line) {
            continue;
        }
        if intent_hits
            .iter()
            .any(|hit| hit.abs_diff(index) <= git_diff_intent_context_radius())
        {
            candidates.push((0, index));
        } else if git_diff_semantic_context_line(line).is_some() {
            candidates.push((1, index));
        } else if is_git_diff_changed_detail_line(line) {
            candidates.push((2, index));
        }
    }

    candidates.sort_by_key(|(priority, index)| (*priority, *index));
    let mut selected = BTreeMap::<usize, ()>::new();
    for (_, index) in candidates.into_iter().take(budget) {
        selected.insert(index, ());
    }
    selected
}

fn git_diff_intent_context_radius() -> usize {
    2
}

fn is_git_diff_changed_detail_line(line: &str) -> bool {
    (line.starts_with('+') && !line.starts_with("+++")
        || line.starts_with('-') && !line.starts_with("---"))
        && !line.trim_start_matches(['+', '-']).trim().is_empty()
}

pub(super) fn compact_git_log_stat_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    let lines = command_lines(input);
    let commits = parse_git_log_stat_commits(&lines);
    if commits.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let stat_files = commits
        .iter()
        .map(|commit| commit.stat_lines.len())
        .sum::<usize>();
    let mut output = Vec::new();
    output.push(format!(
        "sum: git log --stat commits={}, stat_files={}",
        commits.len(),
        stat_files,
    ));

    let commit_limit = options
        .max_lines
        .max(24)
        .saturating_div(8)
        .max(2)
        .min(commits.len());
    let per_commit_stat_limit = options
        .max_path_entries
        .max(1)
        .checked_div(commit_limit.max(1))
        .unwrap_or(1)
        .clamp(2, 8);

    for commit in commits.iter().take(commit_limit) {
        output.push(format!(
            "commit: {}",
            truncate_command_line(&commit.header, options.max_line_chars)
        ));
        for meta in commit.metadata.iter().take(2) {
            output.push(format!(
                "  {}",
                truncate_command_line(meta, options.max_line_chars)
            ));
        }
        for subject in commit.subject.iter().take(2) {
            output.push(format!(
                "  subject: {}",
                truncate_command_line(subject, options.max_line_chars)
            ));
        }
        for summary in &commit.stat_summaries {
            output.push(format!(
                "  stat totals: {}",
                truncate_command_line(summary, options.max_line_chars)
            ));
        }
        if !commit.stat_lines.is_empty() {
            output.push(format!("  stat files ({}):", commit.stat_lines.len()));
            push_head_tail_lines(
                &mut output,
                &commit.stat_lines,
                per_commit_stat_limit,
                options.max_line_chars,
                "stat entries",
                "    ",
            );
        }
    }

    if commits.len() > commit_limit {
        output.push(format!(
            "[... omitted {} commits ...]",
            commits.len() - commit_limit
        ));
    }

    finalize_compacted_command_output(CommandOutputKind::GitLog, input, output, options)
}

pub(super) fn compact_search_output(input: &str, options: &CommandOutputCompactOptions) -> String {
    let (files, other) = collect_search_output_matches(input);

    let total_matches = files.values().map(Vec::len).sum::<usize>();
    if total_matches == 0 {
        return smart_truncate_command_output(input, options);
    }

    let mut output = Vec::new();
    output.push(format!(
        "sum: search matches={}, files={}",
        total_matches,
        files.len(),
    ));
    for (path, matches) in files {
        output.push(format!("{path} ({} matches):", matches.len()));
        for search_match in matches
            .iter()
            .take(options.max_search_matches_per_file.max(1))
        {
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
        if matches.len() > options.max_search_matches_per_file.max(1) {
            output.push(format!(
                "  [... {} more matches in this file ...]",
                matches.len() - options.max_search_matches_per_file.max(1),
            ));
        }
    }

    if !other.is_empty() {
        output.push(format!("other lines ({}):", other.len()));
        for line in other.iter().take(4) {
            output.push(format!(
                "  {}",
                truncate_command_line(line, options.max_line_chars)
            ));
        }
        if other.len() > 4 {
            output.push(format!("  [... {} more other lines ...]", other.len() - 4));
        }
    }

    finalize_compacted_command_output(CommandOutputKind::Search, input, output, options)
}

pub(super) fn compact_file_list_output(
    input: &str,
    options: &CommandOutputCompactOptions,
) -> String {
    let entries = collect_file_list_entries(input);

    if entries.is_empty() {
        return smart_truncate_command_output(input, options);
    }

    let mut roots = BTreeMap::<String, usize>::new();
    let mut extensions = BTreeMap::<String, usize>::new();
    for entry in &entries {
        let path = normalize_file_list_path(entry);
        *roots.entry(top_level_path_segment(&path)).or_default() += 1;
        *extensions.entry(path_extension_label(&path)).or_default() += 1;
    }

    let mut output = Vec::new();
    output.push(format!("sum: files entries={}", entries.len()));
    output.push(format_count_map("top roots", &roots, 8));
    output.push(format_count_map("extensions", &extensions, 8));
    output.push("entries:".to_string());

    let max_entries = options.max_path_entries.max(1);
    let head = max_entries.div_ceil(2);
    let tail = max_entries.saturating_sub(head);
    if entries.len() <= max_entries {
        for entry in entries {
            output.push(truncate_command_line(&entry, options.max_line_chars));
        }
    } else {
        for entry in entries.iter().take(head) {
            output.push(truncate_command_line(entry, options.max_line_chars));
        }
        output.push(format!(
            "[... omitted {} file-list entries ...]",
            entries.len().saturating_sub(head + tail),
        ));
        for entry in entries.iter().skip(entries.len().saturating_sub(tail)) {
            output.push(truncate_command_line(entry, options.max_line_chars));
        }
    }

    finalize_compacted_command_output(CommandOutputKind::FileList, input, output, options)
}

pub(super) fn collect_search_output_matches(
    input: &str,
) -> (BTreeMap<String, Vec<SearchMatch>>, Vec<String>) {
    let lines = command_lines(input);
    let mut files: BTreeMap<String, Vec<SearchMatch>> = BTreeMap::new();
    let mut other = Vec::new();
    let mut current_heading_path = None::<String>;

    for line in lines {
        if let Some(search_match) =
            parse_rg_json_match_line(line).or_else(|| parse_search_match_line(line))
        {
            current_heading_path = Some(search_match.path.clone());
            files
                .entry(search_match.path.clone())
                .or_default()
                .push(search_match);
        } else if let Some(search_match) =
            parse_heading_search_match_line(line, current_heading_path.as_deref())
        {
            files
                .entry(search_match.path.clone())
                .or_default()
                .push(search_match);
        } else if let Some(path) = parse_search_heading_line(line) {
            current_heading_path = Some(path);
        } else if looks_like_rg_json_line(line) {
            // Non-match rg JSON records are command metadata, not useful context.
        } else if !line.trim().is_empty() {
            other.push(line.to_string());
        }
    }

    (files, other)
}

pub(super) fn collect_file_list_entries(input: &str) -> Vec<String> {
    command_lines(input)
        .into_iter()
        .filter_map(parse_file_list_entry_line)
        .collect()
}

fn compact_git_diff_stat_output(
    lines: &[&str],
    options: &CommandOutputCompactOptions,
) -> Option<Vec<String>> {
    let stat_lines = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_line(line))
        .copied()
        .collect::<Vec<_>>();
    if stat_lines.is_empty()
        || !lines
            .iter()
            .any(|line| looks_like_git_diff_stat_summary(line))
    {
        return None;
    }

    let summary_lines = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_summary(line))
        .copied()
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    output.push(format!(
        "sum: git diff stat_only entries={}",
        stat_lines.len()
    ));
    for line in summary_lines {
        output.push(format!(
            "stat totals: {}",
            truncate_command_line(line.trim(), options.max_line_chars)
        ));
    }
    output.push("stat files:".to_string());

    let limit = options
        .max_path_entries
        .max(1)
        .min(options.max_lines.max(8));
    let head = limit.div_ceil(2);
    let tail = limit.saturating_sub(head);
    if stat_lines.len() <= limit {
        for line in stat_lines {
            output.push(format!(
                "  {}",
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
    } else {
        for line in stat_lines.iter().take(head) {
            output.push(format!(
                "  {}",
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
        output.push(format!(
            "  [... omitted {} stat entries ...]",
            stat_lines.len().saturating_sub(head + tail)
        ));
        for line in stat_lines
            .iter()
            .skip(stat_lines.len().saturating_sub(tail))
        {
            output.push(format!(
                "  {}",
                truncate_command_line(line.trim(), options.max_line_chars)
            ));
        }
    }

    Some(output)
}
