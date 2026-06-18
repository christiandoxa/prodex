use super::super::gemini_request_extensions::runtime_gemini_extension_context_files;
use super::super::gemini_request_io::{
    runtime_gemini_read_text_limited, runtime_gemini_truncate_to_bytes,
};
use super::gemini_request_util::{runtime_gemini_bool_value, runtime_gemini_env_bool};
use super::{
    RUNTIME_GEMINI_MEMORY_BYTE_LIMIT, runtime_gemini_collect_path_values, runtime_gemini_config_dir,
};
use std::env;
use std::path::Path;

pub(super) fn runtime_gemini_hierarchical_memory(original: &serde_json::Value) -> Option<String> {
    let mut sections = Vec::new();
    let mut budget = RUNTIME_GEMINI_MEMORY_BYTE_LIMIT;
    runtime_gemini_collect_inline_memory_sections(original, &mut sections, &mut budget);
    if runtime_gemini_memory_files_enabled(original) {
        runtime_gemini_collect_standard_memory_files(&mut sections, &mut budget);
    }
    runtime_gemini_collect_extension_memory_files(&mut sections, &mut budget);
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

fn runtime_gemini_collect_inline_memory_sections(
    original: &serde_json::Value,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    for key in [
        "gemini_memory",
        "geminiMemory",
        "gemini_hierarchical_memory",
        "geminiHierarchicalMemory",
    ] {
        if let Some(value) = original.get(key).filter(|value| !value.is_null()) {
            runtime_gemini_collect_memory_value("Request", value, sections, budget);
        }
    }
    for key in [
        "gemini_memory_file",
        "geminiMemoryFile",
        "gemini_memory_files",
        "geminiMemoryFiles",
    ] {
        let mut paths = Vec::new();
        runtime_gemini_collect_path_values(original.get(key), &mut paths);
        for path in paths {
            runtime_gemini_push_memory_file_section("Request File", &path, sections, budget);
        }
    }
}

fn runtime_gemini_collect_memory_value(
    title: &str,
    value: &serde_json::Value,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    match value {
        serde_json::Value::String(text) => {
            runtime_gemini_push_memory_section(title, text, sections, budget);
        }
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                let section_title = match key.as_str() {
                    "global" | "user" | "userMemory" => "Global",
                    "project" | "projectMemory" | "userProjectMemory" => "Project",
                    "private" | "privateProjectMemory" => "Private Project Memory",
                    "extension" | "extensionMemory" => "Extension",
                    _ => key.as_str(),
                };
                runtime_gemini_collect_memory_value(section_title, value, sections, budget);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_gemini_collect_memory_value(title, item, sections, budget);
            }
        }
        _ => {}
    }
}

pub(super) fn runtime_gemini_memory_files_enabled(original: &serde_json::Value) -> bool {
    if runtime_gemini_env_bool("PRODEX_GEMINI_DISABLE_MEMORY") == Some(true)
        || runtime_gemini_env_bool("PRODEX_GEMINI_DISABLE_CONTEXT_FILES") == Some(true)
    {
        return false;
    }
    for key in [
        "gemini_load_memory",
        "geminiLoadMemory",
        "gemini_memory_files_enabled",
        "geminiMemoryFilesEnabled",
    ] {
        if let Some(value) = original.get(key) {
            return runtime_gemini_bool_value(value).unwrap_or(false);
        }
    }
    for key in ["PRODEX_GEMINI_LOAD_MEMORY", "PRODEX_GEMINI_MEMORY"] {
        if let Some(enabled) = runtime_gemini_env_bool(key) {
            return enabled;
        }
    }
    true
}

fn runtime_gemini_collect_standard_memory_files(sections: &mut Vec<String>, budget: &mut usize) {
    if let Some(gemini_home) = runtime_gemini_config_dir() {
        runtime_gemini_push_memory_file_section(
            "Global",
            &gemini_home.join("GEMINI.md"),
            sections,
            budget,
        );
        runtime_gemini_push_memory_file_section(
            "Global Memory Inbox",
            &gemini_home.join("memory").join("INBOX.md"),
            sections,
            budget,
        );
    }
    if let Ok(cwd) = env::current_dir() {
        let mut ancestors = cwd.ancestors().collect::<Vec<_>>();
        ancestors.reverse();
        for directory in ancestors {
            let title = if directory == cwd {
                "Project".to_string()
            } else {
                format!("Project: {}", directory.display())
            };
            runtime_gemini_push_memory_file_section(
                &title,
                &directory.join("GEMINI.md"),
                sections,
                budget,
            );
        }
        runtime_gemini_push_memory_file_section(
            "Private Project Memory",
            &cwd.join(".gemini").join("memory").join("MEMORY.md"),
            sections,
            budget,
        );
        runtime_gemini_push_memory_file_section(
            "Private Project Memory Inbox",
            &cwd.join(".gemini").join("memory").join("INBOX.md"),
            sections,
            budget,
        );
    }
}

fn runtime_gemini_collect_extension_memory_files(sections: &mut Vec<String>, budget: &mut usize) {
    let mut paths = Vec::new();
    if let Some(configured) = env::var_os("PRODEX_GEMINI_EXTENSION_MEMORY") {
        paths.extend(env::split_paths(&configured));
    }
    paths.extend(runtime_gemini_extension_context_files());
    for path in paths {
        runtime_gemini_push_memory_file_section("Extension", &path, sections, budget);
    }
}

fn runtime_gemini_push_memory_file_section(
    title: &str,
    path: &Path,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    let Some(text) = runtime_gemini_read_text_limited(path, *budget) else {
        return;
    };
    let title = format!("{title}: {}", path.display());
    runtime_gemini_push_memory_section(&title, &text, sections, budget);
}

fn runtime_gemini_push_memory_section(
    title: &str,
    text: &str,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    if *budget == 0 {
        return;
    }
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let text = runtime_gemini_truncate_to_bytes(text, *budget);
    *budget = budget.saturating_sub(text.len());
    sections.push(format!("--- {title} ---\n{text}"));
}
