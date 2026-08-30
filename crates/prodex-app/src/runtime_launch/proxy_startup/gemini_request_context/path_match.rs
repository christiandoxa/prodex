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
        #[cfg(windows)]
        if root.as_os_str().is_empty() && component.len() == 2 && component.as_bytes()[1] == b':' {
            root.push(format!("{component}{}", std::path::MAIN_SEPARATOR));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeGeminiGlobError;

#[cfg(feature = "mojo-quota")]
pub(super) fn runtime_gemini_glob_matches(
    pattern: &str,
    path: &str,
) -> Result<bool, RuntimeGeminiGlobError> {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let pattern = pattern.trim_start_matches("./");
    let path = path.trim_start_matches("./");
    prodex_mojo_core::context::gemini_glob_matches(pattern, path)
        .map_err(|_| RuntimeGeminiGlobError)
}

#[cfg(not(feature = "mojo-quota"))]
pub(super) fn runtime_gemini_glob_matches(
    pattern: &str,
    path: &str,
) -> Result<bool, RuntimeGeminiGlobError> {
    Ok(runtime_gemini_glob_matches_rust(pattern, path))
}

#[cfg(any(not(feature = "mojo-quota"), test))]
fn runtime_gemini_glob_matches_rust(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");
    let pattern = pattern.trim_start_matches("./");
    let path = path.trim_start_matches("./");
    runtime_gemini_glob_component_matches_rust(
        &pattern.split('/').collect::<Vec<_>>(),
        &path.split('/').collect::<Vec<_>>(),
    )
}

#[cfg(any(not(feature = "mojo-quota"), test))]
fn runtime_gemini_glob_component_matches_rust(pattern: &[&str], path: &[&str]) -> bool {
    let Some((head, tail)) = pattern.split_first() else {
        return path.is_empty();
    };
    if *head == "**" {
        return runtime_gemini_glob_component_matches_rust(tail, path)
            || path.split_first().is_some_and(|(_, path_tail)| {
                runtime_gemini_glob_component_matches_rust(pattern, path_tail)
            });
    }
    path.split_first().is_some_and(|(path_head, path_tail)| {
        runtime_gemini_glob_segment_matches_rust(head.as_bytes(), path_head.as_bytes())
            && runtime_gemini_glob_component_matches_rust(tail, path_tail)
    })
}

#[cfg(any(not(feature = "mojo-quota"), test))]
fn runtime_gemini_glob_segment_matches_rust(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((&b'*', tail)) => {
            runtime_gemini_glob_segment_matches_rust(tail, text)
                || text.split_first().is_some_and(|(_, text_tail)| {
                    runtime_gemini_glob_segment_matches_rust(pattern, text_tail)
                })
        }
        Some((&b'?', tail)) => text.split_first().is_some_and(|(_, text_tail)| {
            runtime_gemini_glob_segment_matches_rust(tail, text_tail)
        }),
        Some((&literal, tail)) => text.split_first().is_some_and(|(&value, text_tail)| {
            literal.eq_ignore_ascii_case(&value)
                && runtime_gemini_glob_segment_matches_rust(tail, text_tail)
        }),
    }
}

#[test]
fn gemini_glob_root_preserves_absolute_path_root() {
    let root = std::env::temp_dir().join("prodex-gemini-glob-root");
    let pattern = format!(
        "{}{}**{}*.txt",
        root.display(),
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    );

    assert_eq!(runtime_gemini_glob_root(&pattern), root);
}

#[cfg(all(test, feature = "mojo-quota"))]
#[test]
fn gemini_mojo_glob_matches_rust_oracle() {
    for (pattern, path) in [
        ("**/*.rs", "src/lib.rs"),
        ("**/*.rs", "lib.rs"),
        ("src/*.RS", "src/lib.rs"),
        ("src/?ib.rs", "src/lib.rs"),
        ("src/*.rs", "src/nested/lib.rs"),
        ("a/**/b", "a/b"),
        ("a/**/b", "a/x/y/b"),
        ("./src\\*.rs", "./src/lib.rs"),
    ] {
        assert_eq!(
            runtime_gemini_glob_matches(pattern, path).expect("Mojo glob match"),
            runtime_gemini_glob_matches_rust(pattern, path),
            "pattern={pattern:?} path={path:?}"
        );
    }
}
