use super::{
    PRODEX_PROVIDER_CODEX_API_KEY, force_codex_api_key_auth_for_provider_runtime,
    write_provider_runtime_codex_auth,
};
use crate::{ChildProcessPlan, PROVIDER_SECRET_ENV_KEYS, remove_provider_secret_env};
use std::ffi::OsString;
use std::path::PathBuf;

#[test]
fn provider_runtime_auth_sets_local_placeholder_and_removes_upstream_secrets() {
    let mut child = ChildProcessPlan {
        binary: OsString::from("codex"),
        args: Vec::new(),
        codex_home: PathBuf::from("/tmp/prodex-caveman-test"),
        extra_env: vec![
            (OsString::from("OPENAI_API_KEY"), OsString::from("user-key")),
            (
                OsString::from("UNRELATED_CHILD_ENV"),
                OsString::from("keep-me"),
            ),
        ],
        removed_env: vec![OsString::from("EXISTING_REMOVED_ENV")],
        reset_terminal_keyboard_enhancement: false,
    };

    force_codex_api_key_auth_for_provider_runtime(&mut child);
    remove_provider_secret_env(&mut child);

    let values = child
        .extra_env
        .iter()
        .filter(|(key, _)| key == "OPENAI_API_KEY")
        .map(|(_, value)| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(values, vec![PRODEX_PROVIDER_CODEX_API_KEY.to_string()]);
    for key in PROVIDER_SECRET_ENV_KEYS {
        assert!(
            child.removed_env.contains(&OsString::from(key)),
            "provider secret env {key} should be removed"
        );
    }
    assert!(
        child
            .removed_env
            .contains(&OsString::from("EXISTING_REMOVED_ENV"))
    );
    assert!(
        !child
            .removed_env
            .contains(&OsString::from("UNRELATED_CHILD_ENV"))
    );
    assert!(
        child
            .extra_env
            .iter()
            .any(|(key, value)| { key == "UNRELATED_CHILD_ENV" && value == "keep-me" })
    );
}

#[test]
fn provider_runtime_auth_writes_api_key_auth_file() {
    let root = std::env::temp_dir().join(format!(
        "prodex-provider-auth-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("temp home should be created");

    write_provider_runtime_codex_auth(&root).expect("auth file should be written");

    let auth = std::fs::read_to_string(root.join("auth.json")).expect("auth should be read");
    let value: serde_json::Value = serde_json::from_str(&auth).expect("auth should be json");
    assert_eq!(value["auth_mode"], "apikey");
    assert_eq!(value["OPENAI_API_KEY"], PRODEX_PROVIDER_CODEX_API_KEY);
    assert!(value["tokens"].is_null());
    std::fs::remove_dir_all(root).expect("temp home should be removed");
}
