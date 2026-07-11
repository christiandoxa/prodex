#![cfg(test)]

use super::gemini_rewrite_test_support::conversation_store;
use super::runtime_gemini_generate_request_body;
use std::fs;

#[cfg(unix)]
#[test]
fn gemini_read_many_files_does_not_follow_symlinked_context_paths() {
    let root = std::env::temp_dir().join(format!(
        "prodex-gemini-context-symlink-{}",
        std::process::id()
    ));
    let directory = root.join("workspace");
    let outside = root.join("outside");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&directory).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(directory.join("visible.txt"), "visible context").unwrap();
    fs::write(outside.join("secret.txt"), "symlink secret").unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), directory.join("linked.txt")).unwrap();
    std::os::unix::fs::symlink(&outside, directory.join("linked_dir")).unwrap();
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Review context",
        "include_paths": [directory.join("linked.txt")],
        "read_many_files": {
            "include": [format!("{}/**/*", directory.display())],
            "useDefaultExcludes": true
        },
    }))
    .unwrap();

    let translated =
        runtime_gemini_generate_request_body(&body, &conversation_store(), false, None, None)
            .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let text = value["contents"][0]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("visible context"));
    assert!(!text.contains("symlink secret"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_request_translation_expands_at_paths_and_read_many_files() {
    let directory =
        std::env::temp_dir().join(format!("prodex-gemini-context-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let at_path = directory.join("at path.txt");
    let explicit_path = directory.join("explicit.txt");
    let excluded_path = directory.join("excluded.txt");
    fs::write(&at_path, "from at path").unwrap();
    fs::write(&explicit_path, "from read many files").unwrap();
    fs::write(&excluded_path, "must stay excluded").unwrap();
    let body = serde_json::to_vec(&serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": format!("Review @\"{}\"", at_path.display()),
        "read_many_files": {
            "include": [format!("{}/**/*.txt", directory.display())],
            "exclude": [excluded_path],
            "recursive": true,
            "useDefaultExcludes": true
        },
    }))
    .unwrap();

    let translated =
        runtime_gemini_generate_request_body(&body, &conversation_store(), false, None, None)
            .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
    let parts = value["contents"][0]["parts"].as_array().unwrap();
    let text = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(text.contains("from at path"));
    assert!(text.contains("from read many files"));
    assert!(!text.contains("must stay excluded"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn gemini_read_many_files_honors_default_and_ordered_custom_ignore_rules() {
    let directory =
        std::env::temp_dir().join(format!("prodex-gemini-ignore-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let ignored_path = directory.join("ignored.txt");
    let kept_path = directory.join("kept.txt");
    let env_path = directory.join(".env");
    let custom_ignore = directory.join("custom.ignore");
    fs::write(&ignored_path, "must stay ignored").unwrap();
    fs::write(&kept_path, "must stay visible").unwrap();
    fs::write(&env_path, "default excluded secret").unwrap();
    fs::write(&custom_ignore, "*.txt\n!kept.txt\n").unwrap();

    let request = |use_default_excludes| {
        serde_json::to_vec(&serde_json::json!({
            "model": "gemini-2.5-pro",
            "input": "Review context",
            "read_many_files": {
                "include": [format!("{}/**/*", directory.display())],
                "useDefaultExcludes": use_default_excludes,
                "file_filtering_options": {
                    "custom_ignore_file_paths": [custom_ignore.clone()]
                }
            }
        }))
        .unwrap()
    };
    let translate_text = |body: Vec<u8>| {
        let translated =
            runtime_gemini_generate_request_body(&body, &conversation_store(), false, None, None)
                .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&translated.body).unwrap();
        value["contents"][0]["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let defaults = translate_text(request(true));
    assert!(!defaults.contains("must stay ignored"));
    assert!(defaults.contains("must stay visible"));
    assert!(!defaults.contains("default excluded secret"));

    let no_defaults = translate_text(request(false));
    assert!(!no_defaults.contains("must stay ignored"));
    assert!(no_defaults.contains("must stay visible"));
    assert!(no_defaults.contains("default excluded secret"));
    fs::remove_dir_all(directory).unwrap();
}
