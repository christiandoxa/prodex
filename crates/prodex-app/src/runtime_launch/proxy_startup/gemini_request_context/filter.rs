//! Gemini local context ignore/filter rules, including gitignore and geminiignore handling.

use super::super::super::gemini_request_io::runtime_gemini_read_text_limited;
use super::RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT;
use super::path_match::{
    runtime_gemini_context_match_path, runtime_gemini_glob_matches,
    runtime_gemini_glob_segment_matches,
};
use prodex_provider_core::{
    gemini_provider_core_collect_string_values, gemini_provider_core_skip_context_path_name,
};
use std::fs;
use std::path::{Path, PathBuf};

const RUNTIME_GEMINI_IGNORE_BYTE_LIMIT: usize = 64 * 1024;

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
pub(super) struct RuntimeGeminiContextFilter {
    project_root: Option<PathBuf>,
    pub(super) use_default_excludes: bool,
    project_rules: Vec<RuntimeGeminiIgnoreRule>,
}

struct RuntimeGeminiIgnoreRule {
    base_dir: PathBuf,
    pattern: String,
    negated: bool,
    directory_only: bool,
}

impl RuntimeGeminiContextFilter {
    pub(super) fn project_defaults() -> Self {
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

    pub(super) fn from_read_many_object(
        object: &serde_json::Map<String, serde_json::Value>,
    ) -> Self {
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
            gemini_provider_core_collect_string_values(
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

    pub(super) fn is_excluded(&self, path: &Path, excludes: &[String]) -> bool {
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
    let remaining = RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT.saturating_sub(*scanned);
    let entries = entries.take(remaining).collect::<Vec<_>>();
    *scanned = scanned.saturating_add(entries.iter().filter(|entry| entry.is_err()).count());
    let mut entries = entries
        .into_iter()
        .filter_map(|entry| entry.ok())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if *scanned >= RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT {
            return;
        }
        *scanned = scanned.saturating_add(1);
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) || name == ".git" {
            continue;
        }
        if use_default_excludes && gemini_provider_core_skip_context_path_name(name) {
            continue;
        }
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
    let Some(content) = runtime_gemini_read_text_limited(path, RUNTIME_GEMINI_IGNORE_BYTE_LIMIT)
    else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_context_filter_honors_nested_gitignore_base_rules() {
        let directory = std::env::temp_dir().join(format!(
            "prodex-gemini-nested-ignore-{}",
            std::process::id()
        ));
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
    fn gemini_context_filter_caps_ignore_file_reads() {
        let directory =
            std::env::temp_dir().join(format!("prodex-gemini-ignore-limit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let content = format!(
            "#{}\nlate.secret\n",
            "x".repeat(RUNTIME_GEMINI_IGNORE_BYTE_LIMIT)
        );
        let ignore_path = directory.join(".gitignore");
        fs::write(&ignore_path, content).unwrap();

        let mut rules = Vec::new();
        runtime_gemini_load_ignore_rules(&ignore_path, &directory, &directory, &mut rules);

        assert!(rules.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn gemini_context_filter_counts_files_toward_scan_limit() {
        let directory = std::env::temp_dir().join(format!(
            "prodex-gemini-ignore-scan-limit-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("ordinary.txt"), "content").unwrap();

        let mut rules = Vec::new();
        let mut scanned = RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT - 1;
        runtime_gemini_load_nested_gitignore_rules(
            &directory,
            &directory,
            false,
            &mut rules,
            &mut scanned,
        );

        assert_eq!(scanned, RUNTIME_GEMINI_CONTEXT_SCAN_LIMIT);
        assert!(rules.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn gemini_context_filter_does_not_follow_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let directory = std::env::temp_dir().join(format!(
            "prodex-gemini-ignore-symlink-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        symlink(&directory, directory.join("cycle")).unwrap();

        let mut rules = Vec::new();
        let mut scanned = 0;
        runtime_gemini_load_nested_gitignore_rules(
            &directory,
            &directory,
            false,
            &mut rules,
            &mut scanned,
        );

        assert_eq!(scanned, 1);
        assert!(rules.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
}
