use super::super::gemini_request_extensions::runtime_gemini_extension_context_files;
use super::super::gemini_request_io::runtime_gemini_read_text_limited;
use super::RUNTIME_GEMINI_MEMORY_BYTE_LIMIT;
use super::gemini_request_util::runtime_gemini_bool_value;
use crate::RuntimeGeminiConfig;
use prodex_provider_core::{
    gemini_provider_core_collect_path_values, gemini_provider_core_truncate_to_bytes,
};
use std::env;
use std::path::Path;

pub(super) fn runtime_gemini_hierarchical_memory(
    original: &serde_json::Value,
    config: &RuntimeGeminiConfig,
) -> Option<String> {
    let mut sections = Vec::new();
    let mut budget = RUNTIME_GEMINI_MEMORY_BYTE_LIMIT;
    runtime_gemini_collect_inline_memory_sections(original, &mut sections, &mut budget);
    if runtime_gemini_memory_files_enabled_with_config(original, config) {
        runtime_gemini_collect_standard_memory_files(config, &mut sections, &mut budget);
    }
    runtime_gemini_collect_extension_memory_files(config, &mut sections, &mut budget);
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
        gemini_provider_core_collect_path_values(original.get(key), &mut paths);
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

fn runtime_gemini_memory_files_enabled_with_config(
    original: &serde_json::Value,
    config: &RuntimeGeminiConfig,
) -> bool {
    if config.memory_files_disabled {
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
    config.memory_files_default
}

#[cfg(test)]
pub(super) fn runtime_gemini_memory_files_enabled(original: &serde_json::Value) -> bool {
    let config = crate::RuntimeConfig::compatibility_current();
    runtime_gemini_memory_files_enabled_with_config(original, &config.gemini)
}

fn runtime_gemini_collect_standard_memory_files(
    config: &RuntimeGeminiConfig,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    if let Some(gemini_home) = &config.config_dir {
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

fn runtime_gemini_collect_extension_memory_files(
    config: &RuntimeGeminiConfig,
    sections: &mut Vec<String>,
    budget: &mut usize,
) {
    let mut paths = config.extension_memory_paths.clone();
    paths.extend(runtime_gemini_extension_context_files(config));
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
    let text = gemini_provider_core_truncate_to_bytes(text, *budget);
    *budget = budget.saturating_sub(text.len());
    sections.push(format!("--- {title} ---\n{text}"));
}
