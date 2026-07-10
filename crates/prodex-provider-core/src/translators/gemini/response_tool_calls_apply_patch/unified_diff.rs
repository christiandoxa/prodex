//! Gemini unified-diff to apply_patch conversion.

use super::gemini_normalized_newlines;

pub(super) fn gemini_unified_diff_to_apply_patch(input: &str) -> Option<String> {
    let normalized = gemini_normalized_newlines(input);
    let lines = normalized.lines().collect::<Vec<_>>();
    let mut output = vec!["*** Begin Patch".to_string()];
    let mut converted_files = 0usize;
    let mut index = 0usize;
    while index < lines.len() {
        if lines[index].starts_with("diff --git ") {
            let end = gemini_next_diff_section(&lines, index + 1);
            converted_files += gemini_convert_diff_section(&lines[index..end], &mut output);
            index = end;
            continue;
        }
        if gemini_is_unified_file_header(&lines, index) {
            let end = gemini_next_header_or_diff_section(&lines, index + 2);
            converted_files += gemini_convert_diff_section(&lines[index..end], &mut output);
            index = end;
            continue;
        }
        index += 1;
    }
    if converted_files == 0 {
        return None;
    }
    output.push("*** End Patch".to_string());
    Some(output.join("\n"))
}

fn gemini_convert_diff_section(lines: &[&str], output: &mut Vec<String>) -> usize {
    let Some(header_index) =
        (0..lines.len()).find(|index| gemini_is_unified_file_header(lines, *index))
    else {
        return 0;
    };
    let old_path = gemini_unified_diff_path(lines[header_index]);
    let new_path = gemini_unified_diff_path(lines[header_index + 1]);
    let body = &lines[(header_index + 2)..];
    match (old_path, new_path) {
        (None, Some(path)) => gemini_convert_add_file(&path, body, output),
        (Some(path), None) => {
            output.push(format!("*** Delete File: {path}"));
            1
        }
        (Some(old_path), Some(new_path)) => {
            gemini_convert_update_file(&old_path, &new_path, body, output)
        }
        (None, None) => 0,
    }
}

fn gemini_convert_add_file(path: &str, lines: &[&str], output: &mut Vec<String>) -> usize {
    let hunk_start = output.len();
    output.push(format!("*** Add File: {path}"));
    let mut saw_added_line = false;
    for line in lines {
        if line.starts_with('+') && !line.starts_with("+++") {
            output.push((*line).to_string());
            saw_added_line = true;
        }
    }
    if saw_added_line {
        1
    } else {
        output.truncate(hunk_start);
        0
    }
}

fn gemini_convert_update_file(
    old_path: &str,
    new_path: &str,
    lines: &[&str],
    output: &mut Vec<String>,
) -> usize {
    let hunk_start = output.len();
    output.push(format!("*** Update File: {old_path}"));
    if old_path != new_path {
        output.push(format!("*** Move to: {new_path}"));
    }
    let mut saw_change = false;
    let mut saw_hunk = false;
    for line in lines {
        if line.starts_with("@@") {
            output.push(gemini_apply_patch_hunk_header(line));
            saw_hunk = true;
        } else if saw_hunk && gemini_is_apply_patch_change_line(line) {
            output.push((*line).to_string());
            saw_change |= line.starts_with('+') || line.starts_with('-');
        } else if saw_hunk && *line == "\\ No newline at end of file" {
            output.push("*** End of File".to_string());
        }
    }
    if saw_change {
        1
    } else {
        output.truncate(hunk_start);
        0
    }
}

fn gemini_apply_patch_hunk_header(line: &str) -> String {
    let Some(after_second_marker) = line
        .strip_prefix("@@")
        .and_then(|rest| rest.find("@@").map(|index| index + 2))
    else {
        return "@@".to_string();
    };
    let context = line[after_second_marker + 2..].trim();
    if context.is_empty() {
        "@@".to_string()
    } else {
        format!("@@ {context}")
    }
}

fn gemini_next_diff_section(lines: &[&str], start: usize) -> usize {
    (start..lines.len())
        .find(|index| lines[*index].starts_with("diff --git "))
        .unwrap_or(lines.len())
}

fn gemini_next_header_or_diff_section(lines: &[&str], start: usize) -> usize {
    (start..lines.len())
        .find(|index| {
            lines[*index].starts_with("diff --git ") || gemini_is_unified_file_header(lines, *index)
        })
        .unwrap_or(lines.len())
}

fn gemini_is_unified_file_header(lines: &[&str], index: usize) -> bool {
    index + 1 < lines.len()
        && lines[index].starts_with("--- ")
        && lines[index + 1].starts_with("+++ ")
}

fn gemini_unified_diff_path(line: &str) -> Option<String> {
    let path = line
        .strip_prefix("--- ")
        .or_else(|| line.strip_prefix("+++ "))?
        .trim_matches('"');
    if matches!(path, "/dev/null") {
        return None;
    }
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .trim()
        .to_string()
        .into()
}

fn gemini_is_apply_patch_change_line(line: &str) -> bool {
    line.starts_with('+') || line.starts_with('-') || line.starts_with(' ')
}
