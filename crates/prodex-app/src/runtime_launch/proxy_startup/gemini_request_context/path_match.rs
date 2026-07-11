//! Glob and path matching helpers for Gemini local context discovery.

use std::path::{Path, PathBuf};

pub(super) fn runtime_gemini_glob_root(pattern: &str) -> PathBuf {
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

pub(super) fn runtime_gemini_context_match_path(path: &Path, pattern: &str) -> String {
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

pub(super) fn runtime_gemini_path_has_glob(path: &str) -> bool {
    path.contains('*') || path.contains('?')
}

pub(super) fn runtime_gemini_glob_matches(pattern: &str, path: &str) -> bool {
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

pub(super) fn runtime_gemini_glob_segment_matches(pattern: &[u8], text: &[u8]) -> bool {
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
