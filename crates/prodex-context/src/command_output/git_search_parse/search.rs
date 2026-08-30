use super::*;

#[derive(Clone)]
pub(crate) struct SearchMatch {
    pub(crate) path: String,
    pub(crate) line_number: Option<usize>,
    pub(crate) text: String,
}

#[cfg(feature = "mojo")]
use prodex_mojo_core::context::{
    CONTEXT_GIT_SEARCH_DIRECT_MATCH, CONTEXT_GIT_SEARCH_HEADING_MATCH,
    CONTEXT_GIT_SEARCH_HEADING_PATH, CONTEXT_GIT_SEARCH_JSON_LINE, CONTEXT_GIT_SEARCH_JSON_MATCH,
    classify_git_search_line,
};

#[cfg(feature = "mojo")]
struct MojoGitSearchLine {
    flags: i64,
    line_number: Option<usize>,
    path: String,
    text: String,
}

#[cfg(feature = "mojo")]
fn mojo_git_search_line(line: &str, heading_path: Option<&str>) -> Option<MojoGitSearchLine> {
    let capacity = line.len().max(1);
    let mut path_output = vec![0_u8; capacity];
    let mut text_output = vec![0_u8; capacity];
    let result =
        classify_git_search_line(line, heading_path, &mut path_output, &mut text_output).ok()?;
    let output_string = |length: i64, output: &[u8]| {
        if length == -1 {
            return Some(String::new());
        }
        let length = usize::try_from(length).ok()?;
        std::str::from_utf8(output.get(..length)?)
            .ok()
            .map(str::to_owned)
    };
    Some(MojoGitSearchLine {
        flags: result[0],
        line_number: usize::try_from(result[1]).ok(),
        path: output_string(result[2], &path_output)?,
        text: output_string(result[3], &text_output)?,
    })
}

#[cfg(feature = "mojo")]
pub(crate) fn parse_search_match_line(line: &str) -> Option<SearchMatch> {
    let result = mojo_git_search_line(line, None)?;
    (result.flags == CONTEXT_GIT_SEARCH_DIRECT_MATCH).then_some(SearchMatch {
        path: result.path,
        line_number: result.line_number,
        text: result.text,
    })
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn parse_search_match_line(line: &str) -> Option<SearchMatch> {
    parse_search_match_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn parse_search_match_line_rust(line: &str) -> Option<SearchMatch> {
    let separator_start = usize::from(has_windows_drive_prefix(line)) * 2;
    let separator = line[separator_start..].find(':')? + separator_start;
    let (path, rest) = line.split_at(separator);
    let rest = rest.strip_prefix(':')?;
    if path.trim().is_empty() || rest.trim().is_empty() {
        return None;
    }

    let (line_number, text) = parse_search_match_rest(path, rest)?;

    Some(SearchMatch {
        path: normalize_file_list_path(path),
        line_number,
        text: text.trim().to_string(),
    })
}

#[cfg(any(not(feature = "mojo"), test))]
fn parse_search_match_rest<'a>(path: &str, rest: &'a str) -> Option<(Option<usize>, &'a str)> {
    let Some((candidate, after_line)) = rest.split_once(':') else {
        return looks_like_search_path(path).then_some((None, rest));
    };
    if candidate.chars().all(|ch| ch.is_ascii_digit()) {
        let text = after_line
            .split_once(':')
            .filter(|(column, _)| column.chars().all(|ch| ch.is_ascii_digit()))
            .map(|(_, after_column)| after_column)
            .unwrap_or(after_line);
        return Some((candidate.parse::<usize>().ok(), text));
    }
    looks_like_search_path(path).then_some((None, rest))
}

#[cfg(feature = "mojo")]
pub(crate) fn parse_rg_json_match_line(line: &str) -> Option<SearchMatch> {
    let result = mojo_git_search_line(line, None)?;
    (result.flags & CONTEXT_GIT_SEARCH_JSON_MATCH != 0).then_some(SearchMatch {
        path: result.path,
        line_number: result.line_number,
        text: result.text,
    })
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn parse_rg_json_match_line(line: &str) -> Option<SearchMatch> {
    parse_rg_json_match_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn parse_rg_json_match_line_rust(line: &str) -> Option<SearchMatch> {
    if !looks_like_rg_json_line_rust(line) || !json_field_has_string_value(line, "type", "match") {
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
        path: normalize_file_list_path(&path),
        line_number: extract_json_usize_field(line, "line_number"),
        text: text.trim().to_string(),
    })
}

#[cfg(feature = "mojo")]
pub(crate) fn looks_like_rg_json_line(line: &str) -> bool {
    mojo_git_search_line(line, None)
        .is_some_and(|result| result.flags & CONTEXT_GIT_SEARCH_JSON_LINE != 0)
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn looks_like_rg_json_line(line: &str) -> bool {
    looks_like_rg_json_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn looks_like_rg_json_line_rust(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('{')
        && trimmed.contains("\"type\"")
        && (trimmed.contains("\"data\"") || trimmed.contains("\"path\""))
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn json_field_has_string_value(line: &str, field: &str, value: &str) -> bool {
    extract_json_string_field(line, field).is_some_and(|found| found == value)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn extract_json_string_field(input: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\"");
    let after_marker = input.split_once(&marker)?.1;
    let after_colon = after_marker.split_once(':')?.1.trim_start();
    let value = after_colon.strip_prefix('"')?;
    parse_json_string_prefix(value)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn extract_json_usize_field(input: &str, field: &str) -> Option<usize> {
    let marker = format!("\"{field}\"");
    let after_marker = input.split_once(&marker)?.1;
    let after_colon = after_marker.split_once(':')?.1.trim_start();
    let digits = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<usize>().ok())?
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn parse_json_string_prefix(input: &str) -> Option<String> {
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

#[cfg(feature = "mojo")]
pub(crate) fn parse_search_heading_line(line: &str) -> Option<String> {
    let result = mojo_git_search_line(line, None)?;
    (result.flags == CONTEXT_GIT_SEARCH_HEADING_PATH).then_some(result.path)
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn parse_search_heading_line(line: &str) -> Option<String> {
    parse_search_heading_line_rust(line)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn parse_search_heading_line_rust(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let has_non_drive_colon = trimmed.contains(':')
        && !(has_windows_drive_prefix(trimmed) && !trimmed[2..].contains(':'));
    if trimmed.is_empty()
        || trimmed == "--"
        || has_non_drive_colon
        || trimmed.contains("://")
        || trimmed.split_whitespace().count() > 1
    {
        return None;
    }
    parse_file_list_entry_line(trimmed).filter(|path| looks_like_search_path(path))
}

#[cfg(feature = "mojo")]
pub(crate) fn parse_heading_search_match_line(
    line: &str,
    path: Option<&str>,
) -> Option<SearchMatch> {
    let result = mojo_git_search_line(line, path)?;
    (result.flags == CONTEXT_GIT_SEARCH_HEADING_MATCH).then_some(SearchMatch {
        path: path?.to_string(),
        line_number: result.line_number,
        text: result.text,
    })
}

#[cfg(not(feature = "mojo"))]
pub(crate) fn parse_heading_search_match_line(
    line: &str,
    path: Option<&str>,
) -> Option<SearchMatch> {
    parse_heading_search_match_line_rust(line, path)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn parse_heading_search_match_line_rust(
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

pub(crate) fn count_heading_search_matches(lines: &[&str]) -> usize {
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

#[cfg(any(not(feature = "mojo"), test))]
pub(crate) fn looks_like_search_path(path: &str) -> bool {
    path.contains('/') || path.contains('\\') || path.contains('.')
}

#[cfg(any(not(feature = "mojo"), test))]
fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

pub(crate) fn looks_like_file_list_line(line: &str) -> bool {
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
        || ((trimmed.contains('/') || trimmed.contains('\\'))
            && !trimmed.contains("://")
            && !trimmed.contains(' '))
        || looks_like_bare_path_entry(trimmed)
}

pub(crate) fn parse_file_list_entry_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if looks_like_file_list_line(trimmed) {
        return Some(normalize_file_list_path(trimmed));
    }
    parse_ls_listing_path(trimmed)
}

pub(crate) fn looks_like_bare_path_entry(trimmed: &str) -> bool {
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

pub(crate) fn parse_ls_listing_path(trimmed: &str) -> Option<String> {
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

pub(crate) fn looks_like_ls_mode(mode: &str) -> bool {
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

pub(crate) fn normalize_file_list_path(entry: &str) -> String {
    let trimmed = entry.trim();
    for marker in [
        "|-- ",
        "`-- ",
        "\u{251c}\u{2500}\u{2500} ",
        "\u{2514}\u{2500}\u{2500} ",
    ] {
        if let Some((_, path)) = trimmed.rsplit_once(marker) {
            return path.trim().replace('\\', "/");
        }
    }
    trimmed.replace('\\', "/")
}

pub(crate) fn top_level_path_segment(path: &str) -> String {
    let trimmed = path.trim_start_matches("./").trim_start_matches('/');
    trimmed
        .split('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(".")
        .to_string()
}

pub(crate) fn path_extension_label(path: &str) -> String {
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

pub(crate) fn format_count_map(
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

pub(crate) fn is_critical_preserve_line(line: &str) -> bool {
    is_error_signal_line(line)
        || count_file_location_signals(line) > 0
        || is_diff_hunk_line(line)
        || is_test_failure_signal_line(line)
        || is_rust_exit_status_line(line)
        || is_stack_signal_line(line)
        || is_rust_diagnostic_signal_line(line)
        || is_log_level_signal_line(line)
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_search_tests {
    use super::*;

    fn shape(value: Option<SearchMatch>) -> Option<(String, Option<usize>, String)> {
        value.map(|value| (value.path, value.line_number, value.text))
    }

    #[test]
    fn mojo_search_parser_matches_rust_oracle() {
        let plain = [
            "src/lib.rs:10:fn alpha() {}",
            "src/lib.rs:20:5:let beta = true;",
            "C:\\workspace\\prodex\\src\\lib.rs:30:fn gamma() {}",
            "README.md:context helper",
            "not a search result",
            ":10:missing path",
            "src/lib.rs::empty line number",
        ];
        for line in plain {
            assert_eq!(
                shape(parse_search_match_line(line)),
                shape(parse_search_match_line_rust(line)),
                "plain: {line:?}"
            );
        }

        let json = [
            r#"{"type":"match","data":{"path":{"text":"src\\lib.rs"},"lines":{"text":"fn alpha()\n"}},"line_number":10}"#,
            r#"{"type":"begin","data":{"path":{"text":"src/lib.rs"}}}"#,
            r#"{"type":"match","data":{"path":{"text":"unterminated}}"#,
            r#"{"type":"match","data":{"path":{"text":"src/lib.rs"},"lines":{"text":"x\u1234"}},"line_number":999999999999999999999}"#,
        ];
        for line in json {
            assert_eq!(
                shape(parse_rg_json_match_line(line)),
                shape(parse_rg_json_match_line_rust(line)),
                "json: {line:?}"
            );
            assert_eq!(
                looks_like_rg_json_line(line),
                looks_like_rg_json_line_rust(line),
                "json marker: {line:?}"
            );
        }

        let heading = "src/lib.rs";
        assert_eq!(
            parse_search_heading_line(heading),
            parse_search_heading_line_rust(heading)
        );
        assert_eq!(
            shape(parse_heading_search_match_line(
                " 10:5:fn alpha() {}",
                Some(heading),
            )),
            shape(parse_heading_search_match_line_rust(
                " 10:5:fn alpha() {}",
                Some(heading),
            ))
        );

        let mut path_output = [0_u8; 1];
        let mut text_output = [0_u8; 1];
        assert_eq!(
            prodex_mojo_core::context::classify_git_search_line(
                "src/lib.rs:10:needle",
                None,
                &mut path_output,
                &mut text_output,
            ),
            Err(prodex_mojo_core::MojoError::Capacity)
        );
        assert_eq!(
            prodex_mojo_core::context::classify_git_search_line(
                &"x".repeat(prodex_mojo_core::context::CONTEXT_GIT_SEARCH_MAX_BYTES + 1),
                None,
                &mut path_output,
                &mut text_output,
            ),
            Err(prodex_mojo_core::MojoError::InvalidInput)
        );
    }
}
