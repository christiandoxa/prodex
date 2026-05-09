use super::*;

#[derive(Default)]
pub(super) struct GitStatusSummary {
    pub(super) branch: Option<String>,
    pub(super) staged: Vec<String>,
    pub(super) modified: Vec<String>,
    pub(super) deleted: Vec<String>,
    pub(super) renamed: Vec<String>,
    pub(super) conflicted: Vec<String>,
    pub(super) untracked: Vec<String>,
    pub(super) other: Vec<String>,
    pub(super) clean: bool,
}

pub(super) fn is_short_git_status_line(line: &str) -> bool {
    if line.len() < 3 {
        return false;
    }
    let bytes = line.as_bytes();
    let valid_status = |byte: u8| {
        matches!(
            byte,
            b' ' | b'M' | b'A' | b'D' | b'R' | b'C' | b'U' | b'?' | b'!'
        )
    };
    valid_status(bytes[0]) && valid_status(bytes[1]) && bytes[2] == b' '
}

pub(super) fn parse_short_git_status_line(line: &str, summary: &mut GitStatusSummary) {
    if let Some(branch) = line.strip_prefix("## ") {
        summary.branch = Some(branch.trim().to_string());
        return;
    }
    if !is_short_git_status_line(line) {
        if !line.trim().is_empty() {
            summary.other.push(line.trim().to_string());
        }
        return;
    }

    let status = &line[..2];
    let path = line[3..].trim();
    if status == "??" {
        summary.untracked.push(path.to_string());
        return;
    }
    if status.contains('U') {
        summary
            .conflicted
            .push(format!("{} {}", status.trim(), path));
        return;
    }

    let mut chars = status.chars();
    let index = chars.next().unwrap_or(' ');
    let worktree = chars.next().unwrap_or(' ');
    push_short_status_path(index, path, true, summary);
    push_short_status_path(worktree, path, false, summary);
}

pub(super) fn push_short_status_path(
    status: char,
    path: &str,
    index: bool,
    summary: &mut GitStatusSummary,
) {
    match status {
        'M' | 'A' => {
            if index {
                summary.staged.push(format!("{status} {path}"));
            } else {
                summary.modified.push(format!("{status} {path}"));
            }
        }
        'D' => summary.deleted.push(format!("{status} {path}")),
        'R' | 'C' => summary.renamed.push(format!("{status} {path}")),
        '?' => summary.untracked.push(path.to_string()),
        ' ' | '!' => {}
        other => summary.other.push(format!("{other} {path}")),
    }
}

pub(super) fn parse_long_git_status_lines(lines: &[&str], summary: &mut GitStatusSummary) {
    let mut section = GitStatusSection::Other;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("(use ") {
            continue;
        }
        if let Some(branch) = trimmed.strip_prefix("On branch ") {
            summary.branch = Some(branch.trim().to_string());
            continue;
        }
        if trimmed.starts_with("HEAD detached ") {
            summary.branch = Some(trimmed.to_string());
            continue;
        }
        if trimmed.contains("nothing to commit") || trimmed.contains("working tree clean") {
            summary.clean = true;
            continue;
        }
        section = match trimmed {
            "Changes to be committed:" => GitStatusSection::Staged,
            "Changes not staged for commit:" => GitStatusSection::Modified,
            "Untracked files:" => GitStatusSection::Untracked,
            "Unmerged paths:" => GitStatusSection::Conflicted,
            _ => section,
        };
        if trimmed.ends_with(':') {
            continue;
        }

        match section {
            GitStatusSection::Staged => summary.staged.push(parse_long_status_path(trimmed)),
            GitStatusSection::Modified => {
                let parsed = parse_long_status_path(trimmed);
                if parsed.starts_with("deleted:") {
                    summary.deleted.push(parsed);
                } else {
                    summary.modified.push(parsed);
                }
            }
            GitStatusSection::Untracked => summary.untracked.push(trimmed.to_string()),
            GitStatusSection::Conflicted => {
                summary.conflicted.push(parse_long_status_path(trimmed))
            }
            GitStatusSection::Other => {
                if !trimmed.starts_with("Your branch ") {
                    summary.other.push(trimmed.to_string());
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum GitStatusSection {
    Staged,
    Modified,
    Untracked,
    Conflicted,
    Other,
}

pub(super) fn parse_long_status_path(trimmed: &str) -> String {
    trimmed
        .split_once(':')
        .map(|(status, path)| format!("{}: {}", status.trim(), path.trim()))
        .unwrap_or_else(|| trimmed.to_string())
}

pub(super) fn push_item_summary(
    output: &mut Vec<String>,
    label: &str,
    items: &[String],
    limit: usize,
) {
    if items.is_empty() {
        return;
    }

    let mut unique = Vec::<&str>::new();
    for item in items {
        if !unique.iter().any(|existing| *existing == item) {
            unique.push(item);
        }
    }

    let mut rendered = unique
        .iter()
        .take(limit)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if unique.len() > limit {
        rendered.push_str(&format!(" (+{} more)", unique.len() - limit));
    }
    output.push(format!("{label} ({}): {rendered}", unique.len()));
}

pub(super) struct GitDiffSummary {
    pub(super) path: String,
    pub(super) added: usize,
    pub(super) removed: usize,
    pub(super) hunks: usize,
    pub(super) binary: bool,
    pub(super) semantic_contexts: Vec<String>,
}

#[derive(Default)]
pub(super) struct RustDiagnosticSummary {
    pub(super) errors: usize,
    pub(super) warnings: usize,
    pub(super) panics: usize,
    pub(super) root_causes: Vec<String>,
    pub(super) diagnostic_headers: Vec<String>,
    pub(super) locations: Vec<String>,
    pub(super) failed_tests: Vec<String>,
    pub(super) exit_statuses: Vec<String>,
}

impl RustDiagnosticSummary {
    pub(super) fn is_empty(&self) -> bool {
        self.errors == 0
            && self.warnings == 0
            && self.panics == 0
            && self.root_causes.is_empty()
            && self.diagnostic_headers.is_empty()
            && self.locations.is_empty()
            && self.failed_tests.is_empty()
            && self.exit_statuses.is_empty()
    }

    pub(super) fn record_diagnostic(&mut self, severity: RustDiagnosticSeverity, line: &str) {
        match severity {
            RustDiagnosticSeverity::Error => self.errors += 1,
            RustDiagnosticSeverity::Warning => self.warnings += 1,
        }
        push_unique_line(&mut self.diagnostic_headers, line.trim());
        if matches!(severity, RustDiagnosticSeverity::Error) {
            push_unique_truncated_line(&mut self.root_causes, line, 240);
        }
    }

    pub(super) fn record_failed_test(&mut self, test_name: &str) {
        push_unique_line(&mut self.failed_tests, test_name.trim());
        push_unique_line(&mut self.root_causes, test_name.trim());
    }

    pub(super) fn record_location(&mut self, line: &str) {
        push_unique_line(&mut self.locations, line.trim());
    }

    pub(super) fn record_exit_status(&mut self, line: &str) {
        push_unique_line(&mut self.exit_statuses, line.trim());
        push_unique_truncated_line(&mut self.root_causes, line, 240);
    }

    pub(super) fn record_block_signals(&mut self, block: &RustCriticalBlock) {
        for line in &block.lines {
            if is_rust_location_line(line) {
                self.record_location(line);
            }
            if is_rust_panic_line(line) {
                self.panics += 1;
                push_unique_truncated_line(&mut self.root_causes, line, 240);
            }
            if is_rust_exit_status_line(line) {
                self.record_exit_status(line);
            }
            if let Some(test_name) = rust_failure_separator_name(line) {
                self.record_failed_test(test_name);
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum RustDiagnosticSeverity {
    Error,
    Warning,
}

pub(super) struct RustCriticalBlock {
    pub(super) label: String,
    pub(super) lines: Vec<String>,
}

#[derive(Default)]
pub(super) struct CommandDiagnosticSummary {
    pub(super) errors: usize,
    pub(super) stack_markers: usize,
    pub(super) root_causes: Vec<String>,
    pub(super) diagnostic_headers: Vec<String>,
    pub(super) locations: Vec<String>,
    pub(super) failed_tests: Vec<String>,
    pub(super) exit_statuses: Vec<String>,
}

impl CommandDiagnosticSummary {
    pub(super) fn is_empty(&self) -> bool {
        self.errors == 0
            && self.stack_markers == 0
            && self.root_causes.is_empty()
            && self.diagnostic_headers.is_empty()
            && self.locations.is_empty()
            && self.failed_tests.is_empty()
            && self.exit_statuses.is_empty()
    }

    pub(super) fn record_line(&mut self, line: &str) {
        if is_error_signal_line(line) || is_typescript_diagnostic_line(line) {
            self.errors += 1;
            push_unique_truncated_line(&mut self.diagnostic_headers, line, 240);
            push_unique_truncated_line(&mut self.root_causes, line, 240);
        }
        if let Some(test_name) = generic_failed_test_name(line) {
            push_unique_line(&mut self.failed_tests, test_name);
            push_unique_line(&mut self.root_causes, test_name);
        }
        if count_file_location_signals(line) > 0 {
            push_unique_truncated_line(&mut self.locations, line, 240);
        }
        if is_rust_exit_status_line(line) {
            push_unique_truncated_line(&mut self.exit_statuses, line, 240);
            push_unique_truncated_line(&mut self.root_causes, line, 240);
        }
        if is_stack_signal_line(line) {
            self.stack_markers += 1;
            push_unique_truncated_line(&mut self.diagnostic_headers, line, 240);
        }
    }

    pub(super) fn record_block_signals(&mut self, block: &CommandCriticalBlock) {
        for line in &block.lines {
            self.record_line(line);
        }
    }
}

pub(super) struct CommandCriticalBlock {
    pub(super) label: String,
    pub(super) lines: Vec<String>,
}

#[derive(Default)]
pub(super) struct GitLogCommitSummary {
    pub(super) header: String,
    pub(super) metadata: Vec<String>,
    pub(super) subject: Vec<String>,
    pub(super) stat_lines: Vec<String>,
    pub(super) stat_summaries: Vec<String>,
}

pub(super) fn split_git_diff_sections<'a>(lines: &'a [&'a str]) -> Vec<Vec<&'a str>> {
    let mut sections = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        if line.starts_with("diff --git ") && !current.is_empty() {
            sections.push(current);
            current = Vec::new();
        }
        if line.starts_with("diff --git ")
            || !current.is_empty()
            || line.starts_with("--- ")
            || line.starts_with("@@ ")
        {
            current.push(*line);
        }
    }
    if !current.is_empty() {
        sections.push(current);
    }
    sections
}

pub(super) fn summarize_git_diff_section(section: &[&str]) -> GitDiffSummary {
    let mut summary = GitDiffSummary {
        path: git_diff_section_path(section),
        added: 0,
        removed: 0,
        hunks: 0,
        binary: false,
        semantic_contexts: Vec::new(),
    };
    for line in section {
        if line.starts_with("@@ ") {
            summary.hunks += 1;
            if let Some(context) = git_diff_semantic_context_line(line) {
                push_unique_truncated_line(&mut summary.semantic_contexts, &context, 120);
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            summary.added += 1;
            if let Some(context) = git_diff_semantic_context_line(line) {
                push_unique_truncated_line(&mut summary.semantic_contexts, &context, 120);
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            summary.removed += 1;
            if let Some(context) = git_diff_semantic_context_line(line) {
                push_unique_truncated_line(&mut summary.semantic_contexts, &context, 120);
            }
        } else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            summary.binary = true;
        }
    }
    summary
}

pub(super) fn git_diff_section_path(section: &[&str]) -> String {
    for line in section {
        if let Some((_, rhs)) = line.split_once(" b/") {
            return rhs.trim().to_string();
        }
    }
    for line in section {
        if let Some(path) = line.strip_prefix("+++ b/") {
            return path.trim().to_string();
        }
    }
    "unknown".to_string()
}

pub(super) fn is_git_diff_structural_line(line: &str) -> bool {
    line.starts_with("diff --git ")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("@@ ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("old mode ")
        || line.starts_with("new mode ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
        || line.starts_with("similarity index ")
        || line.starts_with("dissimilarity index ")
        || line.starts_with("Binary files ")
        || line.starts_with("GIT binary patch")
}

pub(super) fn is_git_diff_excerpt_structural_line(line: &str, intent_focused: bool) -> bool {
    if line.starts_with("@@ ")
        || line.starts_with("Binary files ")
        || line.starts_with("GIT binary patch")
    {
        return true;
    }
    if intent_focused {
        return false;
    }
    line.starts_with("diff --git ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("old mode ")
        || line.starts_with("new mode ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
}

pub(super) fn git_diff_semantic_context_line(line: &str) -> Option<String> {
    if line.starts_with("@@ ")
        && let Some((_, context)) = line.rsplit_once("@@")
    {
        let context = context.trim();
        if !context.is_empty() {
            return Some(context.to_string());
        }
    }

    let trimmed = line.trim_start_matches(['+', '-', ' ']).trim();
    if trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with('#') && !trimmed.starts_with("#[")
        || trimmed.starts_with('*')
    {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    let semantic_start = [
        "pub fn ",
        "async fn ",
        "fn ",
        "def ",
        "class ",
        "impl ",
        "pub struct ",
        "struct ",
        "pub enum ",
        "enum ",
        "interface ",
        "type ",
        "function ",
        "describe(",
        "it(",
        "test(",
        "#[test]",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let semantic_contains = lower.contains(" test_")
        || lower.contains("::test_")
        || lower.contains(" should ")
        || lower.contains("=>")
            && (lower.contains("test") || lower.contains("describe") || lower.contains("it("));

    (semantic_start || semantic_contains).then(|| trimmed.to_string())
}

pub(super) fn looks_like_git_diff_output(lines: &[&str]) -> bool {
    if lines.iter().any(|line| line.starts_with("diff --git "))
        || lines
            .iter()
            .filter(|line| is_diff_hunk_line(line))
            .take(2)
            .count()
            >= 1
    {
        return true;
    }

    let stat_lines = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_line(line))
        .count();
    let stat_summaries = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_summary(line))
        .count();
    stat_lines > 0 && stat_summaries > 0
}

pub(super) fn looks_like_git_diff_stat_line(line: &str) -> bool {
    let trimmed = line.trim();
    let Some((path, stats)) = trimmed.split_once(" | ") else {
        return false;
    };
    if path.trim().is_empty() || stats.trim().is_empty() {
        return false;
    }
    let stats = stats.trim();
    stats.starts_with("Bin ")
        || stats.chars().any(|ch| ch == '+' || ch == '-')
        || stats
            .split_whitespace()
            .next()
            .is_some_and(|count| count.chars().all(|ch| ch.is_ascii_digit()))
}

pub(super) fn looks_like_git_diff_stat_summary(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains(" file changed")
        || trimmed.contains(" files changed")
        || trimmed.contains(" insertion")
        || trimmed.contains(" deletion")
}

pub(super) fn looks_like_git_log_stat_output(lines: &[&str]) -> bool {
    let commit_headers = lines
        .iter()
        .filter(|line| parse_git_log_commit_header(line).is_some())
        .count();
    if commit_headers == 0 {
        return false;
    }
    let stat_lines = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_line(line))
        .count();
    let stat_summaries = lines
        .iter()
        .filter(|line| looks_like_git_diff_stat_summary(line))
        .count();
    stat_lines > 0 || stat_summaries > 0
}

pub(super) fn parse_git_log_stat_commits(lines: &[&str]) -> Vec<GitLogCommitSummary> {
    let mut commits = Vec::new();
    let mut current = None::<GitLogCommitSummary>;

    for line in lines {
        if let Some(header) = parse_git_log_commit_header(line) {
            if let Some(commit) = current.take()
                && (!commit.stat_lines.is_empty() || !commit.stat_summaries.is_empty())
            {
                commits.push(commit);
            }
            current = Some(GitLogCommitSummary {
                header,
                ..GitLogCommitSummary::default()
            });
            continue;
        }

        let Some(commit) = current.as_mut() else {
            continue;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("Author:") || trimmed.starts_with("Date:") {
            push_unique_line(&mut commit.metadata, trimmed);
        } else if looks_like_git_diff_stat_line(line) {
            push_unique_line(&mut commit.stat_lines, trimmed);
        } else if looks_like_git_diff_stat_summary(line) {
            push_unique_line(&mut commit.stat_summaries, trimmed);
        } else if line.starts_with("    ") && commit.subject.len() < 2 {
            push_unique_line(&mut commit.subject, trimmed);
        }
    }

    if let Some(commit) = current.take()
        && (!commit.stat_lines.is_empty() || !commit.stat_summaries.is_empty())
    {
        commits.push(commit);
    }
    commits
}

pub(super) fn parse_git_log_commit_header(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(hash) = trimmed.strip_prefix("commit ") {
        let hash = hash.split_whitespace().next().unwrap_or_default();
        if looks_like_git_hash(hash) {
            return Some(format!("commit {hash}"));
        }
    }

    let (hash, subject) = trimmed.split_once(' ')?;
    if looks_like_git_hash(hash) && !subject.trim().is_empty() {
        return Some(trimmed.to_string());
    }
    None
}

pub(super) fn looks_like_git_hash(input: &str) -> bool {
    (7..=64).contains(&input.len()) && input.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[derive(Clone)]
pub(crate) struct SearchMatch {
    pub(crate) path: String,
    pub(crate) line_number: Option<usize>,
    pub(crate) text: String,
}

pub(crate) fn parse_search_match_line(line: &str) -> Option<SearchMatch> {
    let (path, rest) = line.split_once(':')?;
    if path.trim().is_empty() || rest.trim().is_empty() {
        return None;
    }

    let (line_number, text) = if let Some((candidate, after_line)) = rest.split_once(':') {
        if candidate.chars().all(|ch| ch.is_ascii_digit()) {
            let text = if let Some((column, after_column)) = after_line.split_once(':') {
                if column.chars().all(|ch| ch.is_ascii_digit()) {
                    after_column
                } else {
                    after_line
                }
            } else {
                after_line
            };
            (candidate.parse::<usize>().ok(), text)
        } else if looks_like_search_path(path) {
            (None, rest)
        } else {
            return None;
        }
    } else if looks_like_search_path(path) {
        (None, rest)
    } else {
        return None;
    };

    Some(SearchMatch {
        path: path.trim().to_string(),
        line_number,
        text: text.trim().to_string(),
    })
}

pub(crate) fn parse_rg_json_match_line(line: &str) -> Option<SearchMatch> {
    if !looks_like_rg_json_line(line) || !json_field_has_string_value(line, "type", "match") {
        return None;
    }

    let path_section = line.split_once("\"path\"")?.1;
    let path = extract_json_string_field(path_section, "text")
        .or_else(|| extract_json_string_field(path_section, "path"))?;
    let lines_section = line.split_once("\"lines\"").map(|(_, section)| section);
    let text = lines_section
        .and_then(|section| extract_json_string_field(section, "text"))
        .unwrap_or_default();
    Some(SearchMatch {
        path: path.trim().to_string(),
        line_number: extract_json_usize_field(line, "line_number"),
        text: text.trim().to_string(),
    })
}

pub(super) fn looks_like_rg_json_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('{')
        && trimmed.contains("\"type\"")
        && (trimmed.contains("\"data\"") || trimmed.contains("\"path\""))
}

pub(super) fn json_field_has_string_value(line: &str, field: &str, value: &str) -> bool {
    extract_json_string_field(line, field).is_some_and(|found| found == value)
}

pub(super) fn extract_json_string_field(input: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\"");
    let after_marker = input.split_once(&marker)?.1;
    let after_colon = after_marker.split_once(':')?.1.trim_start();
    let value = after_colon.strip_prefix('"')?;
    parse_json_string_prefix(value)
}

pub(super) fn extract_json_usize_field(input: &str, field: &str) -> Option<usize> {
    let marker = format!("\"{field}\"");
    let after_marker = input.split_once(&marker)?.1;
    let after_colon = after_marker.split_once(':')?.1.trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<usize>().ok())?
}

pub(super) fn parse_json_string_prefix(input: &str) -> Option<String> {
    let mut output = String::new();
    let mut escaped = false;
    for ch in input.chars() {
        if escaped {
            match ch {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                other => output.push(other),
            }
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some(output),
            other => output.push(other),
        }
    }
    None
}

pub(super) fn parse_search_heading_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed == "--"
        || trimmed.contains(':')
        || trimmed.contains("://")
        || trimmed.split_whitespace().count() > 1
    {
        return None;
    }
    parse_file_list_entry_line(trimmed).filter(|path| looks_like_search_path(path))
}

pub(super) fn parse_heading_search_match_line(
    line: &str,
    path: Option<&str>,
) -> Option<SearchMatch> {
    let path = path?;
    let trimmed = line.trim_start();
    let (candidate, after_line) = trimmed.split_once(':')?;
    if !candidate.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let text = if let Some((column, after_column)) = after_line.split_once(':') {
        if column.chars().all(|ch| ch.is_ascii_digit()) {
            after_column
        } else {
            after_line
        }
    } else {
        after_line
    };
    Some(SearchMatch {
        path: path.to_string(),
        line_number: candidate.parse::<usize>().ok(),
        text: text.trim().to_string(),
    })
}

pub(super) fn count_heading_search_matches(lines: &[&str]) -> usize {
    let mut count = 0usize;
    let mut current_path = None::<String>;
    for line in lines {
        if parse_search_match_line(line).is_some() || parse_rg_json_match_line(line).is_some() {
            current_path = None;
            continue;
        }
        if let Some(path) = parse_search_heading_line(line) {
            current_path = Some(path);
            continue;
        }
        if parse_heading_search_match_line(line, current_path.as_deref()).is_some() {
            count += 1;
        }
    }
    count
}

pub(super) fn looks_like_search_path(path: &str) -> bool {
    path.contains('/') || path.contains('\\') || path.contains('.')
}

pub(super) fn looks_like_file_list_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("[...")
        || trimmed.contains(" directories, ")
        || trimmed.contains("://")
    {
        return false;
    }
    trimmed.starts_with("./")
        || trimmed.starts_with('/')
        || trimmed.starts_with("|-- ")
        || trimmed.starts_with("`-- ")
        || trimmed.contains("\u{251c}\u{2500}\u{2500} ")
        || trimmed.contains("\u{2514}\u{2500}\u{2500} ")
        || (trimmed.contains('/') && !trimmed.contains("://") && !trimmed.contains(' '))
        || looks_like_bare_path_entry(trimmed)
}

pub(crate) fn parse_file_list_entry_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if looks_like_file_list_line(trimmed) {
        return Some(normalize_file_list_path(trimmed));
    }
    parse_ls_listing_path(trimmed)
}

pub(super) fn looks_like_bare_path_entry(trimmed: &str) -> bool {
    if trimmed.is_empty()
        || trimmed.chars().any(char::is_whitespace)
        || trimmed.contains(':')
        || trimmed.starts_with('-')
        || trimmed.starts_with('{')
        || trimmed.starts_with('[')
    {
        return false;
    }

    if matches!(
        trimmed,
        "Cargo.toml"
            | "Cargo.lock"
            | "Makefile"
            | "README"
            | "README.md"
            | "LICENSE"
            | "AGENTS.md"
            | ".gitignore"
    ) {
        return true;
    }

    let Some((_, ext)) = trimmed.rsplit_once('.') else {
        return false;
    };
    !ext.is_empty()
        && ext.len() <= 12
        && ext
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

pub(super) fn parse_ls_listing_path(trimmed: &str) -> Option<String> {
    if trimmed.is_empty() || trimmed.starts_with("total ") {
        return None;
    }
    let first = trimmed.chars().next()?;
    if !matches!(first, '-' | 'd' | 'l' | 'c' | 'b' | 'p' | 's' | 'D') {
        return None;
    }
    let parts = trimmed.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 9 || !looks_like_ls_mode(parts[0]) {
        return None;
    }
    let path = parts[8..].join(" ");
    let path = path.trim();
    if path.is_empty() || path == "." || path == ".." {
        None
    } else {
        Some(path.to_string())
    }
}

pub(super) fn looks_like_ls_mode(mode: &str) -> bool {
    mode.len() >= 10
        && mode.chars().all(|ch| {
            matches!(
                ch,
                '-' | 'd'
                    | 'l'
                    | 'c'
                    | 'b'
                    | 'p'
                    | 's'
                    | 'D'
                    | 'r'
                    | 'w'
                    | 'x'
                    | 'S'
                    | 'T'
                    | 't'
                    | '+'
            )
        })
}

pub(super) fn normalize_file_list_path(entry: &str) -> String {
    let trimmed = entry.trim();
    for marker in [
        "|-- ",
        "`-- ",
        "\u{251c}\u{2500}\u{2500} ",
        "\u{2514}\u{2500}\u{2500} ",
    ] {
        if let Some((_, path)) = trimmed.rsplit_once(marker) {
            return path.trim().to_string();
        }
    }
    trimmed.to_string()
}

pub(super) fn top_level_path_segment(path: &str) -> String {
    let trimmed = path.trim_start_matches("./").trim_start_matches('/');
    trimmed
        .split('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(".")
        .to_string()
}

pub(super) fn path_extension_label(path: &str) -> String {
    let file_name = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches('/');
    file_name
        .rsplit_once('.')
        .and_then(|(_, ext)| {
            let valid = !ext.is_empty()
                && ext.len() <= 12
                && ext
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
            valid.then(|| ext.to_ascii_lowercase())
        })
        .unwrap_or_else(|| "none".to_string())
}

pub(super) fn format_count_map(
    label: &str,
    counts: &BTreeMap<String, usize>,
    limit: usize,
) -> String {
    let mut entries = counts.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(name, count)| (Reverse(**count), (*name).clone()));
    let mut rendered = entries
        .iter()
        .take(limit)
        .map(|(name, count)| format!("{name}={count}"))
        .collect::<Vec<_>>()
        .join(", ");
    if entries.len() > limit {
        rendered.push_str(&format!(" (+{} more)", entries.len() - limit));
    }
    format!("{label}: {rendered}")
}

pub(super) fn is_critical_preserve_line(line: &str) -> bool {
    is_error_signal_line(line)
        || count_file_location_signals(line) > 0
        || is_diff_hunk_line(line)
        || is_test_failure_signal_line(line)
        || is_rust_exit_status_line(line)
        || is_stack_signal_line(line)
        || is_rust_diagnostic_signal_line(line)
        || is_log_level_signal_line(line)
}
