use super::{Fixture, Value, fs, json, write_json};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) fn read_state(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path.join("state.json")).expect("failed to read state.json"))
        .expect("failed to parse state.json")
}

pub(crate) fn read_access_token(codex_home: &Path) -> String {
    serde_json::from_slice::<Value>(
        &fs::read(codex_home.join("auth.json")).expect("failed to read auth.json"),
    )
    .expect("failed to parse auth.json")["tokens"]["access_token"]
        .as_str()
        .expect("access_token should be a string")
        .to_string()
}

pub(crate) fn active_profile(path: &Path) -> String {
    read_state(path)["active_profile"]
        .as_str()
        .expect("active_profile should be a string")
        .to_string()
}

pub(crate) fn runtime_broker_registry_path(prodex_home: &Path) -> Option<PathBuf> {
    fs::read_dir(prodex_home)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("runtime-broker-") && name.ends_with(".json"))
        })
}

pub(crate) fn wait_for_runtime_broker_registry_path(prodex_home: &Path) -> PathBuf {
    crate::test_wait::wait_for_poll(
        "runtime broker registry",
        Duration::from_secs(30),
        Duration::from_millis(10),
        || runtime_broker_registry_path(prodex_home),
    )
}

pub(crate) fn add_managed_profile(fixture: &Fixture, name: &str, account_id: &str) -> PathBuf {
    let home = fixture.prodex_home.join("profiles").join(name);
    fs::create_dir_all(&home).expect("failed to create additional home");
    write_json(
        &home.join("auth.json"),
        &json!({
            "tokens": {
                "access_token": "test-token",
                "account_id": account_id
            }
        }),
    );

    let mut state = read_state(&fixture.prodex_home);
    let profiles = state
        .get_mut("profiles")
        .and_then(Value::as_object_mut)
        .expect("profiles should be an object");
    profiles.insert(
        name.to_string(),
        json!({
            "codex_home": home,
            "managed": true
        }),
    );
    write_json(&fixture.prodex_home.join("state.json"), &state);

    home
}
