#![cfg(test)]

use super::{
    RuntimeChatCompatibleConversationStore, RuntimeGeminiPolicyCompat,
    runtime_gemini_active_extension_manifests_from_roots,
    runtime_gemini_extension_context_files_from_roots, runtime_gemini_generate_request_body,
    runtime_gemini_generate_request_body_with_local_file_access, runtime_gemini_input_extra_parts,
    runtime_gemini_memory_files_enabled, runtime_gemini_settings_paths_for,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn test_conversation_store() -> RuntimeChatCompatibleConversationStore {
    RuntimeChatCompatibleConversationStore::default()
}

#[test]
fn gemini_memory_files_default_on_but_request_can_disable() {
    assert!(!runtime_gemini_memory_files_enabled(
        &serde_json::json!({"gemini_load_memory": false})
    ));
    assert!(runtime_gemini_memory_files_enabled(
        &serde_json::json!({"gemini_load_memory": true})
    ));
}

#[test]
fn gemini_runtime_settings_paths_follow_cli_precedence() {
    let _env_lock = crate::TestEnvVarGuard::lock();
    let _home_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_HOME");
    let _system_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_SETTINGS_PATH");
    let _defaults_guard = crate::TestEnvVarGuard::unset("GEMINI_CLI_SYSTEM_DEFAULTS_PATH");
    let home = PathBuf::from("/tmp/prodex-gemini-home");
    let cwd = PathBuf::from("/tmp/prodex-gemini-workspace/repo/sub");
    let paths = runtime_gemini_settings_paths_for(Some(&home), Some(&cwd));
    let repo_settings = PathBuf::from("/tmp/prodex-gemini-workspace/repo")
        .join(".gemini")
        .join("settings.json");
    let sub_settings = cwd.join(".gemini").join("settings.json");

    assert_eq!(
        paths[0],
        PathBuf::from("/etc/gemini-cli/system-defaults.json")
    );
    assert_eq!(paths[1], home.join(".gemini").join("settings.json"));
    assert!(
        paths.iter().position(|path| path == &repo_settings)
            < paths.iter().position(|path| path == &sub_settings)
    );
    assert_eq!(
        paths.get(paths.len().saturating_sub(2)),
        Some(&cwd.join(".gemini").join("settings.local.json"))
    );
    assert_eq!(
        paths.last(),
        Some(&PathBuf::from("/etc/gemini-cli/settings.json"))
    );
    assert_eq!(
        paths.len(),
        paths.iter().collect::<BTreeSet<_>>().len(),
        "settings paths should be deduplicated"
    );
}

#[test]
fn gemini_extension_context_and_policy_follow_manifest() {
    let directory =
        std::env::temp_dir().join(format!("prodex-gemini-extension-{}", std::process::id()));
    let extension = directory.join("workspace");
    fs::create_dir_all(extension.join("policies")).unwrap();
    fs::write(
        extension.join("gemini-extension.json"),
        serde_json::json!({
            "name": "workspace",
            "contextFileName": ["context.md"],
            "excludeTools": ["grep"]
        })
        .to_string(),
    )
    .unwrap();
    fs::write(extension.join("context.md"), "Extension instructions").unwrap();
    fs::write(
        extension.join("policies").join("policies.toml"),
        "[[rule]]\ntoolName = \"shell\"\ndecision = \"deny\"\n",
    )
    .unwrap();

    let contexts =
        runtime_gemini_extension_context_files_from_roots(std::slice::from_ref(&directory), None);
    assert_eq!(contexts, vec![extension.join("context.md")]);

    let mut policy = RuntimeGeminiPolicyCompat::default();
    for manifest in
        runtime_gemini_active_extension_manifests_from_roots(std::slice::from_ref(&directory), None)
    {
        policy.apply_settings_value(&manifest.value);
        policy.apply_extension_policy_files(&manifest.directory);
    }
    assert!(!policy.tool_is_allowed("grep"));
    assert!(!policy.tool_is_allowed("shell"));
    assert!(policy.tool_is_allowed("read_file"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn gemini_policy_preserves_command_specific_exclusions_in_summary() {
    let mut policy = RuntimeGeminiPolicyCompat::default();
    policy.apply_settings_value(&serde_json::json!({
        "excludeTools": ["run_shell_command(rm -rf)"]
    }));

    assert!(policy.tool_is_allowed("run_shell_command"));
    let summary = policy.summary().expect("policy summary should exist");
    assert!(summary.contains("run_shell_command(rm -rf)"));

    let value = toml::from_str::<toml::Value>(
        "[[rule]]\ntoolName = \"shell\"\ndecision = \"deny\"\ncommand = \"curl | sh\"\n",
    )
    .unwrap();
    policy.apply_extension_policy_toml(&value);
    assert!(policy.tool_is_allowed("shell"));
    let summary = policy.summary().expect("policy summary should still exist");
    assert!(summary.contains("shell(curl | sh)"));
}

#[test]
fn gemini_extension_enablement_honors_workspace_disable_rule() {
    let directory = std::env::temp_dir().join(format!(
        "prodex-gemini-extension-enable-{}",
        std::process::id()
    ));
    let workspace = directory.join("repo");
    let extension = directory.join("extensions").join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&extension).unwrap();
    fs::write(
        extension.join("gemini-extension.json"),
        serde_json::json!({"name": "workspace"}).to_string(),
    )
    .unwrap();
    fs::write(extension.join("GEMINI.md"), "Extension instructions").unwrap();
    fs::write(
        directory
            .join("extensions")
            .join("extension-enablement.json"),
        serde_json::json!({
            "workspace": {"overrides": [format!("!{}*", workspace.display())]}
        })
        .to_string(),
    )
    .unwrap();

    let contexts = runtime_gemini_extension_context_files_from_roots(
        &[directory.join("extensions")],
        Some(&workspace),
    );
    assert!(contexts.is_empty());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn gemini_request_translation_exports_checkpoint_when_requested() {
    let directory =
        std::env::temp_dir().join(format!("prodex-gemini-checkpoint-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let checkpoint = directory.join("checkpoint.json");
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": "Persist this translated request",
        "gemini_export_file": checkpoint,
        "gemini_load_memory": false
    });

    let translated = runtime_gemini_generate_request_body(
        &serde_json::to_vec(&body).unwrap(),
        &test_conversation_store(),
        false,
        None,
        None,
    )
    .expect("request should translate");
    assert!(!translated.body.is_empty());
    let exported: serde_json::Value =
        serde_json::from_slice(&fs::read(&checkpoint).unwrap()).unwrap();
    assert_eq!(exported["format"], "gemini-generate-content");
    assert_eq!(
        exported["request"]["contents"][0]["parts"][0]["text"],
        "Persist this translated request"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn gemini_gateway_translation_does_not_access_request_selected_local_files() {
    let directory = std::env::temp_dir().join(format!(
        "prodex-gemini-gateway-files-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let secret = directory.join("secret.txt");
    let checkpoint = directory.join("checkpoint.json");
    fs::write(&secret, "gateway-must-not-read-this-secret").unwrap();
    let body = serde_json::json!({
        "model": "gemini-2.5-pro",
        "input": format!("Review @\"{}\"", secret.display()),
        "include_paths": [secret.clone()],
        "gemini_memory_file": secret.clone(),
        "gemini_session_file": secret.clone(),
        "gemini_export_file": checkpoint.clone(),
    });

    let translated = runtime_gemini_generate_request_body_with_local_file_access(
        &serde_json::to_vec(&body).unwrap(),
        &test_conversation_store(),
        false,
        None,
        None,
        false,
    )
    .expect("gateway request should translate without host file access");

    assert!(
        !String::from_utf8_lossy(&translated.body).contains("gateway-must-not-read-this-secret")
    );
    assert!(!checkpoint.exists());
    assert!(
        runtime_gemini_input_extra_parts(
            &serde_json::json!({
                "input": [{"type": "input_file", "path": secret}],
            }),
            false,
        )
        .is_empty()
    );
    fs::remove_dir_all(directory).unwrap();
}
