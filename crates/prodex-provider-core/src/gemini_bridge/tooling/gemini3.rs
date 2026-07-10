//! Gemini 3 tool declaration description overrides.

use super::gemini_provider_core_tool_aliases;

pub fn gemini_provider_core_apply_gemini3_tool_declaration_overrides(
    model: &str,
    declarations: &mut [serde_json::Value],
) {
    if !gemini_provider_core_model_uses_gemini3_toolset(model) {
        return;
    }
    for declaration in declarations {
        let Some(name) = declaration
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        if let Some(description) = gemini_provider_core_gemini3_tool_description(&name)
            && let Some(object) = declaration.as_object_mut()
        {
            object.insert(
                "description".to_string(),
                serde_json::Value::String(description.to_string()),
            );
        }
        gemini_provider_core_apply_gemini3_parameter_descriptions(&name, declaration);
    }
}

pub fn gemini_provider_core_model_uses_gemini3_toolset(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    model.contains("gemini-3") || model == "auto" || model.contains("auto-gemini-3")
}

pub fn gemini_provider_core_gemini3_tool_description(name: &str) -> Option<&'static str> {
    let aliases = gemini_provider_core_tool_aliases(name);
    if aliases.contains("read_file") {
        return Some(
            "Read a file with targeted, surgical ranges. Prefer start_line and end_line when only part of the file is needed.",
        );
    }
    if aliases.contains("read_many_files") {
        return Some(
            "Read multiple files by paths or globs; honor git, Gemini, custom ignore rules, and default heavy-directory excludes.",
        );
    }
    if aliases.contains("grep") || aliases.contains("rg") || aliases.contains("rip_grep") {
        return Some(
            "Search text fast with ripgrep-style behavior. Use this before broad reads when locating symbols or literals.",
        );
    }
    if aliases.contains("exec_command") || aliases.contains("run_shell_command") {
        return Some(
            "Run a shell command. Prefer non-interactive commands and continue background sessions until the needed output is available.",
        );
    }
    if aliases.contains("write_file") {
        return Some(
            "Write complete file content. Do not use placeholders; preserve unrelated user changes.",
        );
    }
    if aliases.contains("apply_patch") || aliases.contains("replace") || aliases.contains("edit") {
        return Some(
            "Apply targeted file edits with exact context. Keep edits narrow and use this instead of describing changes. For Add File patches, every content line must start with '+', for example: *** Begin Patch\\n*** Add File: note.txt\\n+hello\\n*** End Patch.",
        );
    }
    None
}

fn gemini_provider_core_apply_gemini3_parameter_descriptions(
    name: &str,
    declaration: &mut serde_json::Value,
) {
    let aliases = gemini_provider_core_tool_aliases(name);
    if aliases.contains("read_file") {
        gemini_provider_core_set_parameter_description(
            declaration,
            "start_line",
            "1-based first line to read when a targeted range is enough.",
        );
        gemini_provider_core_set_parameter_description(
            declaration,
            "end_line",
            "1-based last line to read, inclusive.",
        );
    }
    if aliases.contains("grep") || aliases.contains("rg") || aliases.contains("rip_grep") {
        gemini_provider_core_set_parameter_description(
            declaration,
            "pattern",
            "Literal or regex pattern to search for.",
        );
    }
    if aliases.contains("apply_patch") || aliases.contains("replace") || aliases.contains("edit") {
        declaration["parameters"]["properties"]["input"]["description"] =
            serde_json::Value::String(
                "Full apply_patch grammar string. For Add File, prefix every new file content line with '+'."
                    .to_string(),
            );
    }
}

fn gemini_provider_core_set_parameter_description(
    declaration: &mut serde_json::Value,
    parameter: &str,
    description: &str,
) {
    let Some(properties) = declaration
        .get_mut("parameters")
        .and_then(|parameters| parameters.get_mut("properties"))
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    let Some(property) = properties
        .get_mut(parameter)
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    property
        .entry("description".to_string())
        .or_insert_with(|| serde_json::Value::String(description.to_string()));
}
