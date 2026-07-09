use super::super::gemini_request_io::runtime_gemini_path_has_symlink_component;
use super::gemini_request_local_context::{
    RUNTIME_GEMINI_CONTEXT_FILE_LIMIT, RuntimeGeminiFileReadBudget,
    runtime_gemini_part_from_local_path, runtime_gemini_resolve_local_path,
    runtime_gemini_skip_context_path_name,
};
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

const RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT: usize = 2048;
const RUNTIME_GEMINI_IGNORE_FILE_BYTE_LIMIT: usize = 256 * 1024;
const RUNTIME_GEMINI_DEFAULT_CONTEXT_EXCLUDES: &[&str] = &[
    "**/node_modules/**",
    "**/.git/**",
    "**/bower_components/**",
    "**/.svn/**",
    "**/.hg/**",
    "**/.vscode/**",
    "**/.idea/**",
    "**/dist/**",
    "**/build/**",
    "**/coverage/**",
    "**/__pycache__/**",
    "**/*.bin",
    "**/*.exe",
    "**/*.dll",
    "**/*.so",
    "**/*.dylib",
    "**/*.class",
    "**/*.jar",
    "**/*.war",
    "**/*.zip",
    "**/*.tar",
    "**/*.gz",
    "**/*.bz2",
    "**/*.rar",
    "**/*.7z",
    "**/*.pak",
    "**/*.rpa",
    "**/*.doc",
    "**/*.docx",
    "**/*.xls",
    "**/*.xlsx",
    "**/*.ppt",
    "**/*.pptx",
    "**/*.odt",
    "**/*.ods",
    "**/*.odp",
    "**/*.pyc",
    "**/*.pyo",
    "**/.DS_Store",
    "**/.env",
    "**/GEMINI.md",
];
#[derive(Default)]
struct RuntimeGeminiContextFilter {
    project_root: Option<PathBuf>,
    use_default_excludes: bool,
    project_rules: Vec<RuntimeGeminiIgnoreRule>,
}

struct RuntimeGeminiIgnoreRule {
    base_dir: PathBuf,
    pattern: String,
    negated: bool,
    directory_only: bool,
}

pub(super) fn runtime_gemini_collect_explicit_file_parts(
    original: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    for key in [
        "include_paths",
        "includePaths",
        "read_many_files",
        "readManyFiles",
        "gemini_context_files",
        "geminiContextFiles",
    ] {
        if let Some(value) = original.get(key) {
            runtime_gemini_collect_file_reference_value(value, parts, budget);
        }
    }
}

fn runtime_gemini_collect_file_reference_value(
    value: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    match value {
        serde_json::Value::String(path) => {
            if let Some(part) = runtime_gemini_part_from_local_path(Path::new(path), None, budget) {
                parts.push(part);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                runtime_gemini_collect_file_reference_value(item, parts, budget);
            }
        }
        serde_json::Value::Object(object) => {
            if object.contains_key("include") {
                runtime_gemini_collect_read_many_files_object(object, parts, budget);
                return;
            }
            if let Some(path) = object
                .get("path")
                .or_else(|| object.get("file_path"))
                .or_else(|| object.get("filePath"))
                .and_then(serde_json::Value::as_str)
            {
                let mime_type = object
                    .get("mime_type")
                    .or_else(|| object.get("mimeType"))
                    .and_then(serde_json::Value::as_str);
                if let Some(part) =
                    runtime_gemini_part_from_local_path(Path::new(path), mime_type, budget)
                {
                    parts.push(part);
                }
            }
        }
        _ => {}
    }
}

fn runtime_gemini_collect_read_many_files_object(
    object: &serde_json::Map<String, serde_json::Value>,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    let mut includes = Vec::new();
    runtime_gemini_collect_string_values(object.get("include"), &mut includes);
    let mut excludes = Vec::new();
    runtime_gemini_collect_string_values(object.get("exclude"), &mut excludes);
    let filter = RuntimeGeminiContextFilter::from_read_many_object(object);
    for include in includes {
        if budget.files >= RUNTIME_GEMINI_CONTEXT_FILE_LIMIT {
            break;
        }
        if runtime_gemini_path_has_glob(&include) {
            runtime_gemini_collect_glob_file_parts(&include, &excludes, &filter, parts, budget);
        } else if !filter.is_excluded(Path::new(&include), &excludes)
            && let Some(part) =
                runtime_gemini_part_from_local_path(Path::new(&include), None, budget)
        {
            parts.push(part);
        }
    }
}

pub(in super::super) fn runtime_gemini_collect_string_values(
    value: Option<&serde_json::Value>,
    values: &mut Vec<String>,
) {
    match value {
        Some(serde_json::Value::String(value)) => values.push(value.to_string()),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_gemini_collect_string_values(Some(item), values);
            }
        }
        _ => {}
    }
}

fn runtime_gemini_collect_glob_file_parts(
    pattern: &str,
    excludes: &[String],
    filter: &RuntimeGeminiContextFilter,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    let root = runtime_gemini_glob_root(pattern);
    let Some(root) = runtime_gemini_resolve_local_path(&root) else {
        return;
    };
    if runtime_gemini_path_has_symlink_component(&root) {
        return;
    }
    let mut candidates = Vec::new();
    let mut scanned = 0;
    runtime_gemini_collect_context_candidates(
        &root,
        filter.use_default_excludes,
        &mut candidates,
        &mut scanned,
    );
    candidates.sort();
    for candidate in candidates {
        if budget.files >= RUNTIME_GEMINI_CONTEXT_FILE_LIMIT {
            break;
        }
        let candidate_match_path = runtime_gemini_context_match_path(&candidate, pattern);
        if !runtime_gemini_glob_matches(pattern, &candidate_match_path)
            || filter.is_excluded(&candidate, excludes)
        {
            continue;
        }
        if let Some(part) = runtime_gemini_part_from_local_path(&candidate, None, budget) {
            parts.push(part);
        }
    }
}

fn runtime_gemini_collect_context_candidates(
    path: &Path,
    use_default_excludes: bool,
    candidates: &mut Vec<PathBuf>,
    scanned: &mut usize,
) {
    if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut entries = entries.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
            break;
        }
        *scanned = scanned.saturating_add(1);
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if use_default_excludes && runtime_gemini_skip_context_path_name(name) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            runtime_gemini_collect_context_candidates(
                &path,
                use_default_excludes,
                candidates,
                scanned,
            );
        } else if metadata.is_file() {
            candidates.push(path);
        }
    }
}

fn runtime_gemini_glob_root(pattern: &str) -> PathBuf {
    let normalized = pattern.replace('\\', "/");
    let mut root = PathBuf::new();
    for component in normalized.split('/') {
        if runtime_gemini_path_has_glob(component) {
            break;
        }
        if component.is_empty() {
            if root.as_os_str().is_empty() {
                root.push(Path::new("/"));
            }
            continue;
        }
        root.push(component);
    }
    if root.as_os_str().is_empty() {
        PathBuf::from(".")
    } else if root.extension().is_some() {
        root.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        root
    }
}

fn runtime_gemini_context_match_path(path: &Path, pattern: &str) -> String {
    let match_path = if Path::new(pattern).is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .ok()
            .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
            .unwrap_or_else(|| path.to_path_buf())
    };
    match_path.to_string_lossy().replace('\\', "/")
}

impl RuntimeGeminiContextFilter {
    fn project_defaults() -> Self {
        let Some(project_root) = std::env::current_dir().ok() else {
            return Self {
                project_root: None,
                use_default_excludes: true,
                project_rules: Vec::new(),
            };
        };
        let mut project_rules = Vec::new();
        runtime_gemini_load_ignore_rules(
            &project_root.join(".gitignore"),
            &project_root,
            &project_root,
            &mut project_rules,
        );
        runtime_gemini_load_nested_gitignore_rules(
            &project_root,
            &project_root,
            true,
            &mut project_rules,
            &mut 0,
        );
        runtime_gemini_load_ignore_rules(
            &project_root.join(".geminiignore"),
            &project_root,
            &project_root,
            &mut project_rules,
        );
        Self {
            project_root: Some(project_root),
            use_default_excludes: true,
            project_rules,
        }
    }

    fn from_read_many_object(object: &serde_json::Map<String, serde_json::Value>) -> Self {
        let use_default_excludes = object
            .get("useDefaultExcludes")
            .or_else(|| object.get("use_default_excludes"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let filtering = object
            .get("file_filtering_options")
            .or_else(|| object.get("fileFilteringOptions"))
            .and_then(serde_json::Value::as_object);
        let respect_git_ignore = filtering
            .and_then(|options| {
                options
                    .get("respect_git_ignore")
                    .or_else(|| options.get("respectGitIgnore"))
            })
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let respect_gemini_ignore = filtering
            .and_then(|options| {
                options
                    .get("respect_gemini_ignore")
                    .or_else(|| options.get("respectGeminiIgnore"))
            })
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let Some(project_root) = std::env::current_dir().ok() else {
            return Self {
                project_root: None,
                use_default_excludes,
                project_rules: Vec::new(),
            };
        };
        let mut project_rules = Vec::new();
        if respect_git_ignore {
            runtime_gemini_load_ignore_rules(
                &project_root.join(".gitignore"),
                &project_root,
                &project_root,
                &mut project_rules,
            );
            runtime_gemini_load_nested_gitignore_rules(
                &project_root,
                &project_root,
                use_default_excludes,
                &mut project_rules,
                &mut 0,
            );
        }
        if respect_gemini_ignore {
            runtime_gemini_load_ignore_rules(
                &project_root.join(".geminiignore"),
                &project_root,
                &project_root,
                &mut project_rules,
            );
        }
        if let Some(filtering) = filtering {
            let mut custom_paths = Vec::new();
            runtime_gemini_collect_string_values(
                filtering
                    .get("custom_ignore_file_paths")
                    .or_else(|| filtering.get("customIgnoreFilePaths")),
                &mut custom_paths,
            );
            for path in custom_paths {
                let path = PathBuf::from(path);
                let path = if path.is_absolute() {
                    path
                } else {
                    project_root.join(path)
                };
                let base_dir = path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| project_root.clone());
                runtime_gemini_load_ignore_rules(
                    &path,
                    &base_dir,
                    &project_root,
                    &mut project_rules,
                );
            }
        }
        Self {
            project_root: Some(project_root),
            use_default_excludes,
            project_rules,
        }
    }

    fn is_excluded(&self, path: &Path, excludes: &[String]) -> bool {
        let match_path = self.match_path(path, "");
        if excludes.iter().any(|exclude| {
            let explicit_match_path = self.match_path(path, exclude);
            runtime_gemini_ignore_pattern_matches(exclude, &explicit_match_path, false)
        }) {
            return true;
        }
        if self.use_default_excludes
            && RUNTIME_GEMINI_DEFAULT_CONTEXT_EXCLUDES
                .iter()
                .any(|exclude| runtime_gemini_ignore_pattern_matches(exclude, &match_path, false))
        {
            return true;
        }
        let mut ignored = false;
        for rule in &self.project_rules {
            if runtime_gemini_ignore_rule_matches(rule, &match_path) {
                ignored = !rule.negated;
            }
        }
        ignored
    }

    fn match_path(&self, path: &Path, pattern: &str) -> String {
        if Path::new(pattern).is_absolute() {
            return path.to_string_lossy().replace('\\', "/");
        }
        if let Some(project_root) = self.project_root.as_deref() {
            return path
                .strip_prefix(project_root)
                .ok()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| path.to_path_buf())
                .to_string_lossy()
                .replace('\\', "/");
        }
        runtime_gemini_context_match_path(path, pattern)
    }
}

fn runtime_gemini_load_nested_gitignore_rules(
    directory: &Path,
    project_root: &Path,
    use_default_excludes: bool,
    rules: &mut Vec<RuntimeGeminiIgnoreRule>,
    scanned: &mut usize,
) {
    if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut entries = entries.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
            return;
        }
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == ".git" {
            continue;
        }
        if use_default_excludes && runtime_gemini_skip_context_path_name(name) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        *scanned = scanned.saturating_add(1);
        runtime_gemini_load_ignore_rules(&path.join(".gitignore"), &path, project_root, rules);
        runtime_gemini_load_nested_gitignore_rules(
            &path,
            project_root,
            use_default_excludes,
            rules,
            scanned,
        );
    }
}

fn runtime_gemini_load_ignore_rules(
    path: &Path,
    base_dir: &Path,
    project_root: &Path,
    rules: &mut Vec<RuntimeGeminiIgnoreRule>,
) {
    let Some(content) = runtime_gemini_read_ignore_file(path) else {
        return;
    };
    let base_dir = base_dir
        .strip_prefix(project_root)
        .unwrap_or(base_dir)
        .to_path_buf();
    for raw in content.lines() {
        let raw = raw.trim_start();
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let (negated, pattern) = raw
            .strip_prefix('!')
            .map(|pattern| (true, pattern))
            .unwrap_or((false, raw));
        let pattern = pattern.trim();
        if pattern.is_empty() {
            continue;
        }
        rules.push(RuntimeGeminiIgnoreRule {
            base_dir: base_dir.clone(),
            pattern: pattern.trim_end_matches('/').replace('\\', "/"),
            negated,
            directory_only: pattern.ends_with('/'),
        });
    }
}

fn runtime_gemini_read_ignore_file(path: &Path) -> Option<String> {
    if runtime_gemini_path_has_symlink_component(path) {
        return None;
    }
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > RUNTIME_GEMINI_IGNORE_FILE_BYTE_LIMIT as u64 {
        return None;
    }
    let file = fs::File::open(path).ok()?;
    let opened_metadata = file.metadata().ok()?;
    if !runtime_gemini_same_ignore_file(&metadata, &opened_metadata) {
        return None;
    }

    let mut bytes = Vec::new();
    file.take((RUNTIME_GEMINI_IGNORE_FILE_BYTE_LIMIT as u64).saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > RUNTIME_GEMINI_IGNORE_FILE_BYTE_LIMIT {
        return None;
    }
    String::from_utf8(bytes).ok()
}

#[cfg(unix)]
fn runtime_gemini_same_ignore_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == after.dev() && before.ino() == after.ino()
}

#[cfg(not(unix))]
fn runtime_gemini_same_ignore_file(_before: &fs::Metadata, _after: &fs::Metadata) -> bool {
    true
}

fn runtime_gemini_ignore_rule_matches(rule: &RuntimeGeminiIgnoreRule, path: &str) -> bool {
    let base_dir = rule.base_dir.to_string_lossy().replace('\\', "/");
    let base_dir = base_dir
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string();
    let path = path
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    let scoped_path = if base_dir.is_empty() {
        path.as_str()
    } else if path == base_dir {
        ""
    } else if let Some(suffix) = path.strip_prefix(&format!("{base_dir}/")) {
        suffix
    } else {
        return false;
    };
    runtime_gemini_ignore_pattern_matches(&rule.pattern, scoped_path, rule.directory_only)
}

fn runtime_gemini_ignore_pattern_matches(pattern: &str, path: &str, directory_only: bool) -> bool {
    let pattern = pattern
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    let path = path
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    if pattern.is_empty() || path.is_empty() {
        return false;
    }
    if pattern.contains('/') {
        if runtime_gemini_glob_matches(&pattern, &path) {
            return true;
        }
        return directory_only
            && path
                .strip_prefix(&pattern)
                .is_some_and(|suffix| suffix.starts_with('/'));
    }
    let components = path.split('/').collect::<Vec<_>>();
    let component_limit = if directory_only {
        components.len().saturating_sub(1)
    } else {
        components.len()
    };
    components[..component_limit].iter().any(|component| {
        runtime_gemini_glob_segment_matches(pattern.as_bytes(), component.as_bytes())
    })
}

fn runtime_gemini_path_has_glob(path: &str) -> bool {
    path.contains('*') || path.contains('?')
}

fn runtime_gemini_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let pattern = pattern.trim_start_matches("./");
    let path = path.trim_start_matches("./");
    runtime_gemini_glob_component_matches(
        &pattern.split('/').collect::<Vec<_>>(),
        &path.split('/').collect::<Vec<_>>(),
    )
}

fn runtime_gemini_glob_component_matches(pattern: &[&str], path: &[&str]) -> bool {
    let Some((head, tail)) = pattern.split_first() else {
        return path.is_empty();
    };
    if *head == "**" {
        return runtime_gemini_glob_component_matches(tail, path)
            || path.split_first().is_some_and(|(_, path_tail)| {
                runtime_gemini_glob_component_matches(pattern, path_tail)
            });
    }
    path.split_first().is_some_and(|(path_head, path_tail)| {
        runtime_gemini_glob_segment_matches(head.as_bytes(), path_head.as_bytes())
            && runtime_gemini_glob_component_matches(tail, path_tail)
    })
}

fn runtime_gemini_glob_segment_matches(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((&b'*', tail)) => {
            runtime_gemini_glob_segment_matches(tail, text)
                || text.split_first().is_some_and(|(_, text_tail)| {
                    runtime_gemini_glob_segment_matches(pattern, text_tail)
                })
        }
        Some((&b'?', tail)) => text
            .split_first()
            .is_some_and(|(_, text_tail)| runtime_gemini_glob_segment_matches(tail, text_tail)),
        Some((&literal, tail)) => text.split_first().is_some_and(|(&value, text_tail)| {
            literal.eq_ignore_ascii_case(&value)
                && runtime_gemini_glob_segment_matches(tail, text_tail)
        }),
    }
}

pub(super) fn runtime_gemini_collect_at_path_parts(
    original: &serde_json::Value,
    parts: &mut Vec<serde_json::Value>,
    budget: &mut RuntimeGeminiFileReadBudget,
) {
    let mut texts = Vec::new();
    runtime_gemini_collect_input_texts(original.get("input"), &mut texts);
    let filter = RuntimeGeminiContextFilter::project_defaults();
    for text in texts {
        for path in runtime_gemini_at_paths_from_text(&text) {
            if !filter.is_excluded(&path, &[])
                && let Some(part) = runtime_gemini_part_from_local_path(&path, None, budget)
            {
                parts.push(part);
            }
        }
    }
}

fn runtime_gemini_collect_input_texts(value: Option<&serde_json::Value>, texts: &mut Vec<String>) {
    match value {
        Some(serde_json::Value::String(text)) => texts.push(text.clone()),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                runtime_gemini_collect_input_texts(Some(item), texts);
            }
        }
        Some(serde_json::Value::Object(object)) => {
            if let Some(text) = object
                .get("text")
                .or_else(|| object.get("content"))
                .and_then(serde_json::Value::as_str)
            {
                texts.push(text.to_string());
            }
            runtime_gemini_collect_input_texts(object.get("content"), texts);
        }
        _ => {}
    }
}

fn runtime_gemini_at_paths_from_text(text: &str) -> Vec<PathBuf> {
    let bytes = text.as_bytes();
    let mut paths = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'@' {
            index += 1;
            continue;
        }
        let mut start = index.saturating_add(1);
        let mut end = start;
        if let Some(quote @ (b'"' | b'\'' | b'`')) = bytes.get(start).copied() {
            start = start.saturating_add(1);
            end = start;
            while end < bytes.len() && bytes[end] != quote {
                end += 1;
            }
            index = end.saturating_add(1);
        } else {
            while end < bytes.len()
                && !bytes[end].is_ascii_whitespace()
                && !matches!(
                    bytes[end],
                    b',' | b';' | b':' | b')' | b']' | b'}' | b'<' | b'>'
                )
            {
                end += 1;
            }
            index = end;
        }
        if start < end
            && let Some(token) = text.get(start..end)
            && !token.contains('@')
        {
            let path = PathBuf::from(token);
            if path.exists() {
                paths.push(path);
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_context_filter_honors_nested_gitignore_base_rules() {
        let directory = runtime_gemini_context_test_dir("nested-ignore");
        let nested = directory.join("nested");
        let other = directory.join("other");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&other).unwrap();
        fs::write(nested.join(".gitignore"), "*.log\n!keep.log\n").unwrap();
        fs::write(nested.join("ignored.log"), "nested ignored").unwrap();
        fs::write(nested.join("keep.log"), "nested kept").unwrap();
        fs::write(other.join("ignored.log"), "other kept").unwrap();

        let mut rules = Vec::new();
        let mut scanned = 0;
        runtime_gemini_load_nested_gitignore_rules(
            &directory,
            &directory,
            false,
            &mut rules,
            &mut scanned,
        );
        let filter = RuntimeGeminiContextFilter {
            project_root: Some(directory.clone()),
            use_default_excludes: false,
            project_rules: rules,
        };

        assert!(filter.is_excluded(&nested.join("ignored.log"), &[]));
        assert!(!filter.is_excluded(&nested.join("keep.log"), &[]));
        assert!(!filter.is_excluded(&other.join("ignored.log"), &[]));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn gemini_context_ignore_rules_reject_oversize_file() {
        let directory = runtime_gemini_context_test_dir("oversize-ignore");
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join(".gitignore"),
            vec![b'a'; RUNTIME_GEMINI_IGNORE_FILE_BYTE_LIMIT + 1],
        )
        .unwrap();
        let mut rules = Vec::new();

        runtime_gemini_load_ignore_rules(
            &directory.join(".gitignore"),
            &directory,
            &directory,
            &mut rules,
        );

        assert!(rules.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn gemini_context_ignore_rules_reject_symlink_file() {
        let directory = runtime_gemini_context_test_dir("symlink-ignore");
        let outside = runtime_gemini_context_test_dir("symlink-ignore-outside");
        fs::create_dir_all(&directory).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("ignore"), "*.secret\n").unwrap();
        std::os::unix::fs::symlink(outside.join("ignore"), directory.join(".gitignore")).unwrap();
        let mut rules = Vec::new();

        runtime_gemini_load_ignore_rules(
            &directory.join(".gitignore"),
            &directory,
            &directory,
            &mut rules,
        );

        assert!(rules.is_empty());
        fs::remove_dir_all(directory).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    fn runtime_gemini_context_test_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "prodex-gemini-context-{name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
