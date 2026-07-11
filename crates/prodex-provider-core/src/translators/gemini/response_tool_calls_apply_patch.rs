use serde_json::Value;

mod unified_diff;

use self::unified_diff::gemini_unified_diff_to_apply_patch;

pub(crate) fn gemini_custom_apply_patch_input(args_value: &Value) -> String {
    if let Some(input) = gemini_explicit_apply_patch_input(args_value) {
        return gemini_normalize_apply_patch_input(input).unwrap_or_else(|| input.to_string());
    }
    if let Some(input) = gemini_edit_args_to_apply_patch(args_value) {
        return input;
    }
    if let Some(input) = gemini_content_apply_patch_input(args_value) {
        return gemini_normalize_apply_patch_input(input).unwrap_or_else(|| input.to_string());
    }
    let input = args_value.to_string();
    gemini_normalize_apply_patch_input(&input).unwrap_or(input)
}

fn gemini_explicit_apply_patch_input(args_value: &Value) -> Option<&str> {
    match args_value {
        Value::String(input) => Some(input),
        Value::Object(object) => ["input", "patch", "diff", "text"]
            .iter()
            .find_map(|key| object.get(*key).and_then(Value::as_str)),
        _ => None,
    }
}

fn gemini_content_apply_patch_input(args_value: &Value) -> Option<&str> {
    args_value
        .as_object()?
        .get("content")
        .and_then(Value::as_str)
}

fn gemini_normalize_apply_patch_input(input: &str) -> Option<String> {
    gemini_extract_apply_patch_block(input).or_else(|| gemini_unified_diff_to_apply_patch(input))
}

fn gemini_extract_apply_patch_block(input: &str) -> Option<String> {
    let normalized = gemini_normalized_newlines(input);
    let lines = normalized.lines().collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with("*** Begin Patch"))?;
    let end = lines[start..]
        .iter()
        .position(|line| line.trim_start().starts_with("*** End Patch"))?
        + start;
    Some(gemini_repair_apply_patch_block(
        &lines[start..=end].join("\n"),
    ))
}

fn gemini_repair_apply_patch_block(input: &str) -> String {
    let normalized = gemini_normalized_newlines(input);
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
        if in_add_file && !line.starts_with('+') {
            output.push(format!("+{line}"));
            continue;
        }
        output.push(line.to_string());
    }
    output.join("\n")
}

fn gemini_edit_args_to_apply_patch(args_value: &Value) -> Option<String> {
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
        (Some(""), Some(new_string)) => Some(gemini_add_file_patch(path, new_string)),
        (Some(old_string), Some(new_string)) => {
            Some(gemini_update_file_patch(path, old_string, new_string))
        }
        (None, Some(new_string)) => Some(gemini_add_file_patch(path, new_string)),
        _ => None,
    }
}

fn gemini_add_file_patch(path: &str, new_string: &str) -> String {
    let mut lines = vec![
        "*** Begin Patch".to_string(),
        format!("*** Add File: {path}"),
    ];
    if new_string.is_empty() {
        lines.push("+".to_string());
    } else {
        lines.extend(
            gemini_normalized_newlines(new_string)
                .lines()
                .map(|line| format!("+{line}")),
        );
    }
    lines.push("*** End Patch".to_string());
    lines.join("\n")
}

fn gemini_update_file_patch(path: &str, old_string: &str, new_string: &str) -> String {
    let mut lines = vec![
        "*** Begin Patch".to_string(),
        format!("*** Update File: {path}"),
        "@@".to_string(),
    ];
    lines.extend(
        gemini_normalized_newlines(old_string)
            .lines()
            .map(|line| format!("-{line}")),
    );
    lines.extend(
        gemini_normalized_newlines(new_string)
            .lines()
            .map(|line| format!("+{line}")),
    );
    lines.push("*** End Patch".to_string());
    lines.join("\n")
}

pub(super) fn gemini_normalized_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}
