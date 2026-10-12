use serde_json::Value;

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
    prodex_mojo_core::gemini_tooling_policy::gemini_apply_patch_block(input)
        .expect("Mojo Gemini apply-patch block normalization failed")
        .or_else(|| {
            prodex_mojo_core::gemini_tooling_policy::gemini_unified_diff_to_apply_patch(input)
                .expect("Mojo Gemini unified-diff conversion failed")
        })
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

fn gemini_normalized_newlines(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}
