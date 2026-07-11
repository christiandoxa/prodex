use super::*;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct CommandPathAlias {
    alias: String,
    prefix: String,
}

pub(super) fn canonicalize_compacted_command_paths(
    original: &str,
    output: &str,
    kind: CommandOutputKind,
) -> String {
    if !command_output_kind_allows_repo_relative_paths(kind) {
        return output.to_string();
    }

    let aliases = repeated_repo_path_aliases(original, output);
    if aliases.is_empty() {
        return output.to_string();
    }

    if let Some(rewritten) = rewrite_absolute_paths_with_aliases(output, &aliases)
        && critical_signal_self_check(output, &rewritten).passed()
    {
        return rewritten;
    }

    // Compatibility fallback: older compaction removed repeated repo prefixes entirely.
    let mut relative = output.to_string();
    for alias in aliases {
        relative = replace_absolute_path_prefix(&relative, &alias.prefix);
    }
    if relative != output && critical_signal_self_check(output, &relative).passed() {
        relative
    } else {
        output.to_string()
    }
}

fn command_output_kind_allows_repo_relative_paths(kind: CommandOutputKind) -> bool {
    matches!(
        kind,
        CommandOutputKind::GitStatus
            | CommandOutputKind::GitDiff
            | CommandOutputKind::RustDiagnostics
            | CommandOutputKind::Diagnostics
            | CommandOutputKind::GitLog
            | CommandOutputKind::Search
            | CommandOutputKind::FileList
            | CommandOutputKind::NoisySuccess
            | CommandOutputKind::Plain
    )
}

fn repeated_repo_path_aliases(original: &str, output: &str) -> Vec<CommandPathAlias> {
    let mut prefixes = repeated_repo_relative_path_prefixes(original);
    for prefix in repeated_repo_relative_path_prefixes(output) {
        push_unique_line(&mut prefixes, &prefix);
    }
    prefixes.retain(|prefix| count_absolute_path_prefix_occurrences(output, prefix) >= 2);
    prefixes.sort_by_key(|prefix| Reverse(prefix.len()));

    let mut selected = Vec::<String>::new();
    for prefix in prefixes {
        if selected
            .iter()
            .any(|existing| path_prefix_contains(existing, &prefix))
        {
            continue;
        }
        selected.push(prefix);
    }

    selected
        .into_iter()
        .enumerate()
        .map(|(index, prefix)| CommandPathAlias {
            alias: if index == 0 {
                "$REPO".to_string()
            } else {
                format!("$PATH{index}")
            },
            prefix,
        })
        .collect()
}

fn rewrite_absolute_paths_with_aliases(
    output: &str,
    aliases: &[CommandPathAlias],
) -> Option<String> {
    let mut rewritten = output.to_string();
    for alias in aliases {
        rewritten =
            replace_absolute_path_prefix_with_alias(&rewritten, &alias.prefix, &alias.alias);
    }
    if rewritten == output {
        return None;
    }

    let mapping = aliases
        .iter()
        .map(|alias| format!("{}={}", alias.alias, alias.prefix))
        .collect::<Vec<_>>()
        .join(", ");
    Some(insert_path_alias_mapping_line(
        &rewritten,
        &format!("path aliases: {mapping}"),
    ))
}

fn insert_path_alias_mapping_line(output: &str, mapping_line: &str) -> String {
    let mut lines = command_lines(output)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let insert_at = usize::from(
        lines
            .first()
            .is_some_and(|line| is_generated_compaction_header_line(line)),
    );
    lines.insert(insert_at, mapping_line.to_string());
    lines_to_text(lines)
}

fn repeated_repo_relative_path_prefixes(input: &str) -> Vec<String> {
    let cwd_prefixes = repeated_current_directory_prefixes(input);
    if !cwd_prefixes.is_empty() {
        return cwd_prefixes;
    }
    repeated_unambiguous_marker_path_prefixes(input)
}

fn repeated_current_directory_prefixes(input: &str) -> Vec<String> {
    let mut prefixes = Vec::<String>::new();
    if let Ok(cwd) = std::env::current_dir()
        && let Some(prefix) = normalize_absolute_path_prefix(&cwd)
    {
        push_unique_line(&mut prefixes, &prefix);
    }
    if let Some(pwd) = std::env::var_os("PWD").map(PathBuf::from)
        && let Some(prefix) = normalize_absolute_path_prefix(&pwd)
    {
        push_unique_line(&mut prefixes, &prefix);
    }

    prefixes.retain(|prefix| count_absolute_path_prefix_occurrences(input, prefix) >= 2);
    prefixes.sort_by_key(|prefix| Reverse(prefix.len()));

    let mut selected = Vec::<String>::new();
    for prefix in prefixes {
        if selected
            .iter()
            .any(|existing| path_prefix_contains(existing, &prefix))
        {
            continue;
        }
        selected.push(prefix);
    }
    selected
}

fn repeated_unambiguous_marker_path_prefixes(text: &str) -> Vec<String> {
    const REPO_MARKERS: &[&str] = &[
        "/crates/",
        "/src/",
        "/tests/",
        "/benches/",
        "/examples/",
        "/README.md",
        "/Cargo.toml",
    ];

    let mut prefix_counts = BTreeMap::<String, usize>::new();
    for marker in REPO_MARKERS {
        let mut search_start = 0usize;
        while let Some(offset) = text[search_start..].find(marker) {
            let marker_start = search_start + offset;
            if let Some(prefix) = absolute_path_prefix_before_marker(text, marker_start) {
                *prefix_counts.entry(prefix).or_default() += 1;
            }
            search_start = marker_start.saturating_add(marker.len());
            if search_start >= text.len() {
                break;
            }
        }
    }

    let prefixes = prefix_counts
        .into_iter()
        .filter(|(prefix, count)| {
            *count >= 2 && prefix.len() > 1 && canonicalizable_absolute_path_prefix(prefix)
        })
        .map(|(prefix, _)| prefix)
        .collect::<Vec<_>>();
    if prefixes.len() != 1 {
        return Vec::new();
    }
    prefixes
}

fn normalize_absolute_path_prefix(path: &Path) -> Option<String> {
    if !path.is_absolute() {
        return None;
    }
    let prefix = path.display().to_string().replace('\\', "/");
    let prefix = prefix.trim_end_matches('/').to_string();
    if prefix.is_empty() || prefix == "/" {
        return None;
    }
    Some(prefix)
}

fn path_prefix_contains(parent: &str, child: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn count_absolute_path_prefix_occurrences(text: &str, prefix: &str) -> usize {
    absolute_path_prefix_occurrences(text, prefix).len()
}

fn replace_absolute_path_prefix(text: &str, prefix: &str) -> String {
    let occurrences = absolute_path_prefix_occurrences(text, prefix);
    if occurrences.is_empty() {
        return text.to_string();
    }

    let marker_len = prefix.len() + 1;
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for start in occurrences {
        output.push_str(&text[cursor..start]);
        cursor = start + marker_len;
    }
    output.push_str(&text[cursor..]);
    output
}

fn replace_absolute_path_prefix_with_alias(text: &str, prefix: &str, alias: &str) -> String {
    let occurrences = absolute_path_prefix_occurrences(text, prefix);
    if occurrences.is_empty() {
        return text.to_string();
    }

    let marker_len = prefix.len() + 1;
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for start in occurrences {
        output.push_str(&text[cursor..start]);
        output.push_str(alias);
        output.push('/');
        cursor = start + marker_len;
    }
    output.push_str(&text[cursor..]);
    output
}

fn absolute_path_prefix_occurrences(text: &str, prefix: &str) -> Vec<usize> {
    let marker = format!("{prefix}/");
    let mut occurrences = Vec::new();
    let mut search_start = 0usize;
    while let Some(relative_start) = text[search_start..].find(&marker) {
        let start = search_start + relative_start;
        let suffix_start = start + marker.len();
        if is_absolute_path_prefix_boundary(text, start)
            && is_repo_relative_suffix_candidate(&text[suffix_start..])
        {
            occurrences.push(start);
        }
        search_start = suffix_start;
    }
    occurrences
}

fn is_absolute_path_prefix_boundary(text: &str, start: usize) -> bool {
    let Some(previous) = text[..start].chars().next_back() else {
        return true;
    };
    if previous != '/' && previous != '\\' {
        return !previous.is_ascii_alphanumeric();
    }

    let before = &text[..start];
    before.ends_with("a/") || before.ends_with("b/")
}

fn is_repo_relative_suffix_candidate(suffix: &str) -> bool {
    let first_segment = suffix
        .split(|ch: char| {
            ch == '/'
                || ch == '\\'
                || ch == ':'
                || ch == '"'
                || ch == '\''
                || ch == '`'
                || ch == ')'
                || ch == ']'
                || ch == '}'
                || ch.is_whitespace()
        })
        .next()
        .unwrap_or_default();
    !matches!(first_segment, "" | "." | "..")
}

fn absolute_path_prefix_before_marker(text: &str, marker_start: usize) -> Option<String> {
    let before = text.get(..marker_start)?;
    let path_start = before
        .char_indices()
        .rev()
        .find(|(_, ch)| !is_absolute_path_prefix_char(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let prefix = text.get(path_start..marker_start)?;
    (prefix.starts_with('/') && prefix.len() > 1).then(|| prefix.to_string())
}

fn is_absolute_path_prefix_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-')
}

fn canonicalizable_absolute_path_prefix(prefix: &str) -> bool {
    !prefix.contains("/rustc/")
        && !prefix.contains("/.rustup/")
        && !prefix.contains("/.cargo/registry/")
        && !prefix.contains("/.cargo/git/")
}
