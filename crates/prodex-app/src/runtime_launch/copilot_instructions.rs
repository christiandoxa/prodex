use anyhow::{Context, Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

const RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_FILES: usize = 32;
const RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_FILE_BYTES: usize = 64 * 1024;
const RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_TOTAL_BYTES: usize = 128 * 1024;
#[cfg(test)]
use super::proxy_startup::deepseek_rewrite::RuntimeDeepSeekTranslatedRequest;

#[cfg(test)]
pub(crate) const RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER: &str =
    "GitHub Copilot custom instructions compatibility";

pub(crate) fn runtime_copilot_workspace_custom_instructions(root: &Path) -> Result<Option<String>> {
    let mut paths = Vec::new();
    let global = root.join(".github").join("copilot-instructions.md");
    if runtime_copilot_instruction_path_is_regular_file(&global) {
        paths.push(global);
    }
    let scoped_root = root.join(".github").join("instructions");
    runtime_copilot_collect_instruction_paths(&scoped_root, &mut paths)?;
    if paths.is_empty() {
        return Ok(None);
    }
    paths.sort();
    paths.truncate(RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_FILES);

    let mut output = String::new();
    let mut total_bytes = 0usize;
    for path in paths {
        if total_bytes >= RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_TOTAL_BYTES {
            break;
        }
        let relative = path.strip_prefix(root).unwrap_or(path.as_path());
        let max_remaining = RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_TOTAL_BYTES - total_bytes;
        let max_bytes = RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_FILE_BYTES.min(max_remaining);
        let Some(content) = runtime_copilot_read_instruction_file(&path, max_bytes)? else {
            continue;
        };
        let content = content.trim();
        if content.is_empty() {
            continue;
        }
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str("## ");
        output.push_str(&relative.to_string_lossy());
        output.push('\n');
        output.push_str(content);
        total_bytes = total_bytes.saturating_add(content.len());
    }

    if output.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

pub(crate) fn runtime_copilot_init_current_workspace_custom_instructions() {
    let _ = runtime_copilot_workspace_custom_instructions_cache();
}

#[cfg(test)]
pub(crate) fn runtime_copilot_cached_workspace_custom_instructions() -> Option<&'static str> {
    runtime_copilot_workspace_custom_instructions_cache().as_deref()
}

fn runtime_copilot_workspace_custom_instructions_cache() -> &'static Option<Arc<str>> {
    static INSTRUCTIONS: OnceLock<Option<Arc<str>>> = OnceLock::new();
    INSTRUCTIONS.get_or_init(|| {
        std::env::current_dir()
            .ok()
            .and_then(|root| runtime_copilot_workspace_custom_instructions(&root).ok())
            .flatten()
            .map(Arc::<str>::from)
    })
}

fn runtime_copilot_collect_instruction_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    if paths.len() >= RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_FILES
        || !runtime_copilot_instruction_path_is_dir(root)
    {
        return Ok(());
    }
    let mut entries = fs::read_dir(root)
        .with_context(|| format!("failed to read {}", root.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to list {}", root.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if paths.len() >= RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_MAX_FILES {
            break;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            runtime_copilot_collect_instruction_paths(&path, paths)?;
        } else if metadata.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".instructions.md"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn runtime_copilot_read_instruction_file(path: &Path, max_bytes: usize) -> Result<Option<String>> {
    if max_bytes == 0 || !runtime_copilot_instruction_path_is_regular_file(path) {
        return Ok(None);
    }
    let file =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(max_bytes as u64)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut content = String::from_utf8_lossy(&bytes).into_owned();
    if content.len() > max_bytes {
        let mut end = max_bytes.min(content.len());
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        content.truncate(end);
    }
    Ok(Some(content))
}

fn runtime_copilot_instruction_path_is_regular_file(path: &Path) -> bool {
    if runtime_copilot_path_has_symlink_component(path) {
        return false;
    }
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

fn runtime_copilot_instruction_path_is_dir(path: &Path) -> bool {
    if runtime_copilot_path_has_symlink_component(path) {
        return false;
    }
    fs::symlink_metadata(path)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
}

fn runtime_copilot_path_has_symlink_component(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
pub(crate) fn runtime_copilot_apply_custom_instructions(
    translated: &mut RuntimeDeepSeekTranslatedRequest,
    instructions: &str,
) -> Result<()> {
    if instructions.trim().is_empty() {
        return Ok(());
    }
    let mut value = serde_json::from_slice::<serde_json::Value>(&translated.body)
        .context("failed to parse Copilot chat request JSON")?;
    let Some(messages) = value
        .get_mut("messages")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(());
    };
    runtime_copilot_merge_custom_instructions_messages(messages, instructions);
    translated.messages = messages.clone();
    translated.body =
        serde_json::to_vec(&value).context("failed to serialize Copilot chat JSON")?;
    Ok(())
}

#[cfg(test)]
fn runtime_copilot_merge_custom_instructions_messages(
    messages: &mut Vec<serde_json::Value>,
    instructions: &str,
) {
    let content = format!("{RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER}\n\n{instructions}");
    if messages.iter().any(|message| {
        message.get("role").and_then(serde_json::Value::as_str) == Some("system")
            && message
                .get("content")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|existing| {
                    existing.contains(RUNTIME_COPILOT_CUSTOM_INSTRUCTIONS_HEADER)
                })
    }) {
        return;
    }
    if let Some(system) = messages
        .iter_mut()
        .find(|message| message.get("role").and_then(serde_json::Value::as_str) == Some("system"))
    {
        let existing = system
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        system["content"] = serde_json::Value::String(if existing.trim().is_empty() {
            content
        } else {
            format!("{existing}\n\n{content}")
        });
        return;
    }
    messages.insert(
        0,
        serde_json::json!({
            "role": "system",
            "content": content,
        }),
    );
}
