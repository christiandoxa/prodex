use serde_json::Value;

pub(crate) fn gemini_custom_apply_patch_input(args_value: &Value) -> String {
    // Host-only Serde acquisition: compiled Mojo owns first-string aliases
    // and Unicode path trimming without cloning request patch strings.
    let mut fields =
        [None; prodex_mojo_core::gemini_tooling_policy::GEMINI_PATCH_SOURCE_FIELD_COUNT];
    match args_value {
        Value::String(text) => fields[0] = Some(text.as_str()),
        Value::Object(object) => {
            for (slot, key) in [
                (1, "input"),
                (2, "patch"),
                (3, "diff"),
                (4, "text"),
                (5, "file_path"),
                (6, "path"),
                (7, "filename"),
                (8, "old_string"),
                (9, "old"),
                (10, "search"),
                (11, "find"),
                (12, "new_string"),
                (13, "new"),
                (14, "replace"),
                (15, "replacement"),
                (16, "content"),
            ] {
                fields[slot] = object.get(key).and_then(Value::as_str);
            }
        }
        _ => {}
    }
    let plan = prodex_mojo_core::gemini_tooling_policy::gemini_apply_patch_sources(&fields)
        .expect("compiled Mojo Gemini apply-patch source selection failed");
    if let Some(input) = plan.explicit {
        return gemini_normalize_apply_patch_input(input).unwrap_or_else(|| input.to_string());
    }
    if let Some(path) = plan.path
        && let Some(input) =
            prodex_mojo_core::gemini_tooling_policy::gemini_edit_patch(path, plan.old, plan.new)
                .expect("Mojo Gemini edit patch assembly failed")
    {
        return input;
    }
    if let Some(input) = plan.content {
        return gemini_normalize_apply_patch_input(input).unwrap_or_else(|| input.to_string());
    }
    let input = args_value.to_string();
    gemini_normalize_apply_patch_input(&input).unwrap_or(input)
}

fn gemini_normalize_apply_patch_input(input: &str) -> Option<String> {
    prodex_mojo_core::gemini_tooling_policy::gemini_apply_patch_block(input)
        .expect("Mojo Gemini apply-patch block normalization failed")
        .or_else(|| {
            prodex_mojo_core::gemini_tooling_policy::gemini_unified_diff_to_apply_patch(input)
                .expect("Mojo Gemini unified-diff conversion failed")
        })
}
