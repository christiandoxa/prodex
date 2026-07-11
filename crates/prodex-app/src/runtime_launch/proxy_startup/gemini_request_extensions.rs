use super::gemini_request::{
    RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT, RUNTIME_GEMINI_MEMORY_BYTE_LIMIT,
};
use super::gemini_request_io::runtime_gemini_read_text_limited;
use crate::RuntimeGeminiConfig;
use prodex_provider_core::gemini_provider_core_collect_string_values;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub(super) struct RuntimeGeminiExtensionManifest {
    pub(super) directory: PathBuf,
    pub(super) name: String,
    pub(super) value: serde_json::Value,
}

pub(super) fn runtime_gemini_extension_context_files(config: &RuntimeGeminiConfig) -> Vec<PathBuf> {
    let roots = runtime_gemini_extension_roots(config);
    let cwd = env::current_dir().ok();
    runtime_gemini_extension_context_files_from_roots_with_config(
        &roots,
        cwd.as_deref(),
        Some(config),
    )
}

#[cfg(test)]
pub(super) fn runtime_gemini_extension_context_files_from_roots(
    roots: &[PathBuf],
    cwd: Option<&Path>,
) -> Vec<PathBuf> {
    runtime_gemini_extension_context_files_from_roots_with_config(roots, cwd, None)
}

fn runtime_gemini_extension_context_files_from_roots_with_config(
    roots: &[PathBuf],
    cwd: Option<&Path>,
    config: Option<&RuntimeGeminiConfig>,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for extension in
        runtime_gemini_active_extension_manifests_from_roots_with_config(roots, cwd, config)
    {
        for name in runtime_gemini_extension_context_file_names(&extension.value) {
            let Some(path) = runtime_gemini_safe_extension_path(&extension.directory, &name) else {
                continue;
            };
            if !path.is_file() {
                continue;
            }
            let key = path.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key) {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

pub(super) fn runtime_gemini_active_extension_manifests(
    config: &RuntimeGeminiConfig,
) -> Vec<RuntimeGeminiExtensionManifest> {
    let roots = runtime_gemini_extension_roots(config);
    let cwd = env::current_dir().ok();
    runtime_gemini_active_extension_manifests_from_roots_with_config(
        &roots,
        cwd.as_deref(),
        Some(config),
    )
}

#[cfg(test)]
pub(super) fn runtime_gemini_active_extension_manifests_from_roots(
    roots: &[PathBuf],
    cwd: Option<&Path>,
) -> Vec<RuntimeGeminiExtensionManifest> {
    runtime_gemini_active_extension_manifests_from_roots_with_config(roots, cwd, None)
}

fn runtime_gemini_active_extension_manifests_from_roots_with_config(
    roots: &[PathBuf],
    cwd: Option<&Path>,
    config: Option<&RuntimeGeminiConfig>,
) -> Vec<RuntimeGeminiExtensionManifest> {
    let mut manifests = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        if manifests.len() >= RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT {
            break;
        }
        if root.join("gemini-extension.json").is_file() {
            if let Some(manifest) =
                runtime_gemini_load_extension_manifest(root, root.parent(), cwd, config)
                && seen.insert(manifest.name.to_ascii_lowercase())
            {
                manifests.push(manifest);
            }
            continue;
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            if manifests.len() >= RUNTIME_GEMINI_EXTENSION_SCAN_LIMIT {
                break;
            }
            let directory = entry.path();
            if !directory.is_dir() || !directory.join("gemini-extension.json").is_file() {
                continue;
            }
            if let Some(manifest) =
                runtime_gemini_load_extension_manifest(&directory, Some(root), cwd, config)
                && seen.insert(manifest.name.to_ascii_lowercase())
            {
                manifests.push(manifest);
            }
        }
    }
    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    manifests
}

fn runtime_gemini_load_extension_manifest(
    directory: &Path,
    root: Option<&Path>,
    cwd: Option<&Path>,
    config: Option<&RuntimeGeminiConfig>,
) -> Option<RuntimeGeminiExtensionManifest> {
    let text = runtime_gemini_read_text_limited(
        &directory.join("gemini-extension.json"),
        RUNTIME_GEMINI_MEMORY_BYTE_LIMIT,
    )?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })?;
    let root = root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| directory.parent().unwrap_or(directory).to_path_buf());
    if !runtime_gemini_extension_is_enabled(&name, cwd, &root, config) {
        return None;
    }
    Some(RuntimeGeminiExtensionManifest {
        directory: directory.to_path_buf(),
        name,
        value,
    })
}

fn runtime_gemini_extension_roots(config: &RuntimeGeminiConfig) -> Vec<PathBuf> {
    let mut roots = config.extension_dirs.clone();
    if let Some(gemini_home) = &config.config_dir {
        roots.push(gemini_home.join("extensions"));
    }
    roots
}

fn runtime_gemini_extension_context_file_names(manifest: &serde_json::Value) -> Vec<String> {
    let Some(context) = manifest.get("contextFileName") else {
        return vec!["GEMINI.md".to_string()];
    };
    let mut names = Vec::new();
    gemini_provider_core_collect_string_values(Some(context), &mut names);
    if names.is_empty() {
        names.push("GEMINI.md".to_string());
    }
    names
}

fn runtime_gemini_safe_extension_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let relative = relative.trim();
    if relative.is_empty() {
        return None;
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(root.join(path))
}

fn runtime_gemini_extension_is_enabled(
    name: &str,
    cwd: Option<&Path>,
    extension_root: &Path,
    config: Option<&RuntimeGeminiConfig>,
) -> bool {
    if let Some(enabled) = config.and_then(|config| config.extension_enabled_override(name)) {
        return enabled;
    }
    let Some(cwd) = cwd else {
        return true;
    };
    let path = extension_root.join("extension-enablement.json");
    let Some(text) = runtime_gemini_read_text_limited(&path, RUNTIME_GEMINI_MEMORY_BYTE_LIMIT)
    else {
        return true;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return true;
    };
    let Some(overrides) = value
        .get(name)
        .and_then(|extension| extension.get("overrides"))
        .and_then(serde_json::Value::as_array)
    else {
        return true;
    };
    let mut enabled = true;
    for rule in overrides.iter().filter_map(serde_json::Value::as_str) {
        if let Some(disable) = runtime_gemini_extension_override_matches(rule, cwd) {
            enabled = !disable;
        }
    }
    enabled
}

fn runtime_gemini_extension_override_matches(rule: &str, cwd: &Path) -> Option<bool> {
    let mut rule = rule.trim();
    if rule.is_empty() {
        return None;
    }
    let disable = rule.starts_with('!');
    if disable {
        rule = &rule[1..];
    }
    let include_subdirs = rule.ends_with('*');
    if include_subdirs {
        rule = &rule[..rule.len().saturating_sub(1)];
    }
    let rule = runtime_gemini_normalize_extension_override_path(rule);
    let cwd = runtime_gemini_normalize_extension_override_path(&cwd.to_string_lossy());
    let matches = if include_subdirs {
        cwd.starts_with(&rule)
    } else {
        cwd == rule
    };
    matches.then_some(disable)
}

fn runtime_gemini_normalize_extension_override_path(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    if !value.starts_with('/') {
        value.insert(0, '/');
    }
    if !value.ends_with('/') {
        value.push('/');
    }
    value
}
