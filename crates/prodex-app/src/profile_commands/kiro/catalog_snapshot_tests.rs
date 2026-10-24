use super::{
    KIRO_MODEL_CATALOG_FILE, KiroAuthSecret, write_kiro_auth_secret,
    write_kiro_model_catalog_snapshot_with_command,
};
use crate::create_codex_home_if_missing;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn write_fake_native_kiro(root: &Path) -> PathBuf {
    const MODELS: &str = r#"{"models":[{"model_name":"auto","description":"Models chosen by task","model_id":"auto","context_window_tokens":1000000},{"model_name":"catalog-model-a","description":"Catalog model A","model_id":"catalog-model-a","context_window_tokens":200000}],"default_model":"auto"}"#;
    // The native probe fixture needs no interpreter startup within its bounded deadline.
    #[cfg(windows)]
    {
        let command = root.join("fake-kiro-native.cmd");
        fs::write(
            &command,
            format!("@echo off\r\nif not \"%~1\"==\"chat\" exit /b 1\r\nif not \"%~2\"==\"--list-models\" exit /b 1\r\nif not defined KIRO_DATA_DIR exit /b 1\r\nif not \"%KIRO_DATA_DIR%\"==\"%Q_CLI_DATA_DIR%\" exit /b 1\r\necho {MODELS}\r\n"),
        )
        .unwrap();
        command
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let command = root.join("fake-kiro-native");
        fs::write(
            &command,
            format!("#!/bin/sh\n[ \"$1\" = chat ] && [ \"$2\" = --list-models ] || exit 1\n[ -n \"$KIRO_DATA_DIR\" ] && [ \"$KIRO_DATA_DIR\" = \"$Q_CLI_DATA_DIR\" ] || exit 1\nprintf '%s\\n' '{MODELS}'\n"),
        )
        .unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).unwrap();
        command
    }
}

fn write_fake_acp_kiro(root: &Path) -> PathBuf {
    crate::test_support::write_test_python_executable(
        root,
        "fake-kiro-acp",
        r#"#!/usr/bin/env python3
import json, os, sys
assert os.environ['KIRO_DATA_DIR'] == os.environ['Q_CLI_DATA_DIR']
if len(sys.argv) > 1 and sys.argv[1] == 'acp':
    first = json.loads(sys.stdin.readline())
    second = json.loads(sys.stdin.readline())
    assert first["method"] == "initialize"
    assert second["method"] == "session/new"
    print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":True,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[{"id":"kiro-login","name":"Kiro Login","description":"Run 'kiro-cli login'."}],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
    print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"catalog-model-a","availableModels":[{"modelId":"catalog-model-a","name":"Catalog Model A"},{"modelId":"catalog-model-b","name":"Catalog Model B"}]}},"id":1}), flush=True)
    sys.exit(0)
sys.exit(1)
"#,
    )
}

#[test]
fn write_kiro_model_catalog_snapshot_from_native_and_acp_models() {
    let root = std::env::temp_dir().join(format!(
        "prodex-kiro-model-catalog-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("test root should exist");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let codex_home = root.join("codex-home");
    create_codex_home_if_missing(&codex_home).expect("codex home should exist");

    let secret = KiroAuthSecret {
        auth_key: "codewhisperer:odic:token".to_string(),
        auth_kind: "builder-id".to_string(),
        auth_json: serde_json::json!({
            "access_token": "kiro-access-token",
            "region": "us-east-1"
        })
        .to_string(),
        email: Some("kiro-user@example.com".to_string()),
        profile_arn: Some("arn:aws:codewhisperer:us-east-1:123456789012:profile/test".to_string()),
        profile_name: Some("builder-id-test".to_string()),
        start_url: Some("https://view.awsapps.com/start".to_string()),
        region: Some("us-east-1".to_string()),
    };
    for (command, expected) in [
        (
            write_fake_native_kiro(&root),
            serde_json::json!({"models": [
                {"id": "auto", "name": "auto", "object": "model", "owned_by": "kiro-cli",
                 "description": "Models chosen by task", "context_window_tokens": 1_000_000},
                {"id": "catalog-model-a", "name": "catalog-model-a", "object": "model",
                 "owned_by": "kiro-cli", "description": "Catalog model A",
                 "context_window_tokens": 200_000}
            ]}),
        ),
        (
            write_fake_acp_kiro(&root),
            serde_json::json!({"models": [
                {"id": "catalog-model-a", "name": "Catalog Model A", "object": "model",
                 "owned_by": "kiro-cli"},
                {"id": "catalog-model-b", "name": "Catalog Model B", "object": "model",
                 "owned_by": "kiro-cli"}
            ]}),
        ),
    ] {
        write_kiro_model_catalog_snapshot_with_command(&codex_home, &secret, command.as_os_str())
            .expect("model catalog snapshot should be written");
        let catalog_text = fs::read_to_string(codex_home.join(KIRO_MODEL_CATALOG_FILE)).unwrap();
        let value: Value = serde_json::from_str(&catalog_text).unwrap();
        assert_eq!(value, expected, "fixture {}", command.display());
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn failed_kiro_catalog_refresh_preserves_previous_snapshot() {
    let root = std::env::temp_dir().join(format!(
        "prodex-kiro-model-catalog-preserve-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let codex_home = root.join("codex-home");
    create_codex_home_if_missing(&codex_home).unwrap();
    let secret = KiroAuthSecret {
        auth_key: "codewhisperer:odic:token".to_string(),
        auth_kind: "builder-id".to_string(),
        auth_json: serde_json::json!({"access_token":"kiro-access-token"}).to_string(),
        email: Some("example@example.test".to_string()),
        profile_arn: None,
        profile_name: None,
        start_url: None,
        region: None,
    };
    write_kiro_auth_secret(&codex_home, &secret).unwrap();
    let catalog_path = codex_home.join(KIRO_MODEL_CATALOG_FILE);
    let previous = r#"{"models":[{"id":"known-model"}]}"#;
    fs::write(&catalog_path, previous).unwrap();

    assert!(
        write_kiro_model_catalog_snapshot_with_command(
            &codex_home,
            &secret,
            std::ffi::OsStr::new("false"),
        )
        .is_err()
    );
    assert_eq!(fs::read_to_string(catalog_path).unwrap(), previous);
    let _ = fs::remove_dir_all(root);
}
