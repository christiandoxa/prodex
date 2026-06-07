use serde_json::Value;

pub(super) fn runtime_gemini_custom_apply_patch_input(args_value: &Value) -> String {
    if let Some(input) = runtime_gemini_explicit_apply_patch_input(args_value) {
        return runtime_gemini_normalize_apply_patch_input(input)
            .unwrap_or_else(|| input.to_string());
    }
    if let Some(input) = runtime_gemini_edit_args_to_apply_patch(args_value) {
        return input;
    }
    if let Some(input) = runtime_gemini_content_apply_patch_input(args_value) {
        return runtime_gemini_normalize_apply_patch_input(input)
            .unwrap_or_else(|| input.to_string());
    }
    let input = args_value.to_string();
    runtime_gemini_normalize_apply_patch_input(&input).unwrap_or(input)
}

fn runtime_gemini_explicit_apply_patch_input(args_value: &Value) -> Option<&str> {
    match args_value {
        Value::String(input) => Some(input),
        Value::Object(object) => ["input", "patch", "diff", "text"]
            .iter()
            .find_map(|key| object.get(*key).and_then(Value::as_str)),
        _ => None,
    }
}

fn runtime_gemini_content_apply_patch_input(args_value: &Value) -> Option<&str> {
    args_value
        .as_object()?
        .get("content")
        .and_then(Value::as_str)
}

fn runtime_gemini_normalize_apply_patch_input(input: &str) -> Option<String> {
    runtime_gemini_extract_apply_patch_block(input)
        .or_else(|| runtime_gemini_unified_diff_to_apply_patch(input))
}

fn runtime_gemini_extract_apply_patch_block(input: &str) -> Option<String> {
    let normalized = runtime_gemini_normalized_newlines(input);
    let lines = normalized.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with("*** Begin Patch"))?;
    let end = lines[start..]
        .iter()
        .position(|line| line.trim_start().starts_with("*** End Patch"))?
        + start;
    Some(runtime_gemini_repair_apply_patch_block(
        &lines[start..=end].join("\n"),
    ))
}

fn runtime_gemini_repair_apply_patch_block(input: &str) -> String {
    let normalized = runtime_gemini_normalized_newlines(input);
    let mut output = Vec::new();
    let mut in_add_file = false;

    for line in normalized.lines() {
        if line.starts_with("*** Add File: ") {
            in_add_file = true;
            output.push(line.to_string());
            continue;
        }
        if line.starts_with("*** ") {
            in_add_file = false;
            output.push(line.to_string());
            continue;
        }
        if in_add_file && !runtime_gemini_is_apply_patch_add_line(line) {
            output.push(format!("+{line}"));
            continue;
        }
        output.push(line.to_string());
    }

    output.join("\n")
}

fn runtime_gemini_is_apply_patch_add_line(line: &str) -> bool {
    line.starts_with('+')
}

fn runtime_gemini_edit_args_to_apply_patch(args_value: &Value) -> Option<String> {
    let object = args_value.as_object()?;
    let path = ["file_path", "path", "filename"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
        .filter(|path| !path.trim().is_empty())?
        .trim();
    let old_string = ["old_string", "old", "search", "find"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str));
    let new_string = ["new_string", "new", "replace", "replacement", "content"]
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str));

    match (old_string, new_string) {
        (Some(""), Some(new_string)) => Some(runtime_gemini_add_file_patch(path, new_string)),
        (Some(old_string), Some(new_string)) => Some(runtime_gemini_update_file_patch(
            path, None, old_string, new_string,
        )),
        (None, Some(new_string)) => Some(runtime_gemini_add_file_patch(path, new_string)),
        _ => None,
    }
}

fn runtime_gemini_unified_diff_to_apply_patch(input: &str) -> Option<String> {
    let normalized = runtime_gemini_normalized_newlines(input);
    let lines = normalized.lines().collect::<Vec<_>>();
    let mut output = vec!["*** Begin Patch".to_string()];
    let mut converted_files = 0usize;
    let mut index = 0usize;

    while index < lines.len() {
        if lines[index].starts_with("diff --git ") {
            let end = runtime_gemini_next_diff_section(&lines, index + 1);
            converted_files += runtime_gemini_convert_diff_section(&lines[index..end], &mut output);
            index = end;
            continue;
        }
        if runtime_gemini_is_unified_file_header(&lines, index) {
            let end = runtime_gemini_next_header_or_diff_section(&lines, index + 2);
            converted_files += runtime_gemini_convert_diff_section(&lines[index..end], &mut output);
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

fn runtime_gemini_convert_diff_section(lines: &[&str], output: &mut Vec<String>) -> usize {
    let metadata = RuntimeGeminiDiffMetadata::from_lines(lines);
    let Some(header_index) =
        (0..lines.len()).find(|index| runtime_gemini_is_unified_file_header(lines, *index))
    else {
        return 0;
    };
    let old_path = metadata
        .rename_from
        .as_deref()
        .map(str::to_string)
        .or_else(|| runtime_gemini_unified_diff_path(lines[header_index]));
    let new_path = metadata
        .rename_to
        .as_deref()
        .map(str::to_string)
        .or_else(|| runtime_gemini_unified_diff_path(lines[header_index + 1]));
    let body = &lines[(header_index + 2)..];

    match (old_path, new_path) {
        (None, Some(path)) => runtime_gemini_convert_add_file(&path, body, output),
        (Some(path), None) => {
            output.push(format!("*** Delete File: {path}"));
            1
        }
        (Some(old_path), Some(new_path)) => {
            runtime_gemini_convert_update_file(&old_path, &new_path, body, output)
        }
        (None, None) => 0,
    }
}

fn runtime_gemini_convert_add_file(path: &str, lines: &[&str], output: &mut Vec<String>) -> usize {
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

fn runtime_gemini_convert_update_file(
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
            output.push(runtime_gemini_apply_patch_hunk_marker(line));
            saw_hunk = true;
        } else if saw_hunk && runtime_gemini_is_apply_patch_change_line(line) {
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

fn runtime_gemini_add_file_patch(path: &str, content: &str) -> String {
    let mut output = vec![
        "*** Begin Patch".to_string(),
        format!("*** Add File: {path}"),
    ];
    output.extend(
        runtime_gemini_normalized_newlines(content)
            .lines()
            .map(|line| format!("+{line}")),
    );
    output.push("*** End Patch".to_string());
    output.join("\n")
}

fn runtime_gemini_update_file_patch(
    path: &str,
    move_to: Option<&str>,
    old_string: &str,
    new_string: &str,
) -> String {
    let mut output = vec![
        "*** Begin Patch".to_string(),
        format!("*** Update File: {path}"),
    ];
    if let Some(move_to) = move_to {
        output.push(format!("*** Move to: {move_to}"));
    }
    output.push("@@".to_string());
    output.extend(
        runtime_gemini_normalized_newlines(old_string)
            .lines()
            .map(|line| format!("-{line}")),
    );
    output.extend(
        runtime_gemini_normalized_newlines(new_string)
            .lines()
            .map(|line| format!("+{line}")),
    );
    output.push("*** End Patch".to_string());
    output.join("\n")
}

fn runtime_gemini_next_diff_section(lines: &[&str], start: usize) -> usize {
    lines[start..]
        .iter()
        .position(|line| line.starts_with("diff --git "))
        .map(|offset| start + offset)
        .unwrap_or(lines.len())
}

fn runtime_gemini_next_header_or_diff_section(lines: &[&str], start: usize) -> usize {
    let mut index = start;
    while index < lines.len() {
        if lines[index].starts_with("diff --git ")
            || runtime_gemini_is_unified_file_header(lines, index)
        {
            return index;
        }
        index += 1;
    }
    lines.len()
}

fn runtime_gemini_is_unified_file_header(lines: &[&str], index: usize) -> bool {
    lines
        .get(index)
        .is_some_and(|line| line.starts_with("--- "))
        && lines
            .get(index + 1)
            .is_some_and(|line| line.starts_with("+++ "))
}

fn runtime_gemini_unified_diff_path(line: &str) -> Option<String> {
    let path = line.get(4..)?.trim();
    let path = path.split('\t').next().unwrap_or(path).trim();
    runtime_gemini_clean_diff_path(path)
}

fn runtime_gemini_clean_diff_path(path: &str) -> Option<String> {
    if path == "/dev/null" {
        return None;
    }
    let path = runtime_gemini_unquote_diff_path(path);
    let path = path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(&path)
        .trim()
        .to_string();
    if path.is_empty() { None } else { Some(path) }
}

fn runtime_gemini_unquote_diff_path(path: &str) -> String {
    let path = path.trim();
    if !(path.len() >= 2 && path.starts_with('"') && path.ends_with('"')) {
        return path.to_string();
    }
    let mut output = String::new();
    let mut chars = path[1..path.len() - 1].chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => output.push('\t'),
            Some('n') => output.push('\n'),
            Some('r') => output.push('\r'),
            Some('"') => output.push('"'),
            Some('\\') => output.push('\\'),
            Some(other) => output.push(other),
            None => output.push('\\'),
        }
    }
    output
}

fn runtime_gemini_apply_patch_hunk_marker(line: &str) -> String {
    let Some(rest) = line.strip_prefix("@@") else {
        return "@@".to_string();
    };
    let Some(end) = rest.find("@@") else {
        return "@@".to_string();
    };
    let context = rest[(end + 2)..].trim();
    if context.is_empty() {
        "@@".to_string()
    } else {
        format!("@@ {context}")
    }
}

fn runtime_gemini_is_apply_patch_change_line(line: &str) -> bool {
    (line.starts_with(' ') || line.starts_with('+') || line.starts_with('-'))
        && !line.starts_with("+++")
        && !line.starts_with("---")
}

fn runtime_gemini_normalized_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

#[derive(Default)]
struct RuntimeGeminiDiffMetadata {
    rename_from: Option<String>,
    rename_to: Option<String>,
}

impl RuntimeGeminiDiffMetadata {
    fn from_lines(lines: &[&str]) -> Self {
        let mut metadata = Self::default();
        for line in lines {
            if let Some(path) = line.strip_prefix("rename from ") {
                metadata.rename_from = runtime_gemini_clean_diff_path(path.trim());
            } else if let Some(path) = line.strip_prefix("rename to ") {
                metadata.rename_to = runtime_gemini_clean_diff_path(path.trim());
            }
        }
        metadata
    }
}
