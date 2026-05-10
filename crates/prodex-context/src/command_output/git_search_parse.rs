use super::*;
mod search;
pub(crate) use search::*;

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
