use super::{TestDir, UsageServer, Value, fs, json};
use std::path::{Path, PathBuf};

pub(crate) struct Fixture {
    pub(crate) _temp_dir: TestDir,
    pub(crate) usage_server: UsageServer,
    pub(crate) usage_base_url: String,
    pub(crate) prodex_home: PathBuf,
    pub(crate) shared_codex_home: PathBuf,
    pub(crate) main_home: PathBuf,
    pub(crate) second_home: PathBuf,
    pub(crate) codex_log: PathBuf,
    pub(crate) codex_args_log: PathBuf,
    pub(crate) codex_stdin_log: PathBuf,
    pub(crate) codex_bin: PathBuf,
}

pub(crate) fn setup_fixture() -> Fixture {
    let temp_dir = TestDir::new();
    let usage_server = UsageServer::start();
    let usage_base_url = usage_server.base_url();
    let prodex_home = temp_dir.path.join("prodex-home");
    let shared_codex_home = temp_dir.path.join("shared-codex-home");
    let homes_root = temp_dir.path.join("homes");
    let bin_root = temp_dir.path.join("bin");
    let main_home = homes_root.join("main");
    let second_home = homes_root.join("second");
    let codex_log = temp_dir.path.join("codex-home.log");
    let codex_args_log = temp_dir.path.join("codex-args.log");
    let codex_stdin_log = temp_dir.path.join("codex-stdin.log");
    let codex_bin = bin_root.join("codex");

    fs::create_dir_all(&prodex_home).expect("failed to create prodex home");
    fs::create_dir_all(&shared_codex_home).expect("failed to create shared codex home");
    fs::create_dir_all(&main_home).expect("failed to create main home");
    fs::create_dir_all(&second_home).expect("failed to create second home");
    fs::create_dir_all(&bin_root).expect("failed to create bin dir");

    write_json(
        &prodex_home.join("state.json"),
        &json!({
            "active_profile": "main",
            "profiles": {
                "main": {
                    "codex_home": main_home,
                    "managed": true
                },
                "second": {
                    "codex_home": second_home,
                    "managed": true
                }
            }
        }),
    );

    write_json(
        &main_home.join("auth.json"),
        &json!({
            "tokens": {
                "access_token": "test-token",
                "account_id": "main-account"
            }
        }),
    );
    write_json(
        &second_home.join("auth.json"),
        &json!({
            "tokens": {
                "access_token": "test-token",
                "account_id": "second-account"
            }
        }),
    );

    write_executable(
        &codex_bin,
        r#"#!/bin/sh
printf '%s\n' "$CODEX_HOME" > "$TEST_CODEX_LOG"
if [ -n "$TEST_CODEX_LOG_APPEND" ]; then
  printf '%s\n' "$CODEX_HOME" >> "$TEST_CODEX_LOG_APPEND"
fi
if [ -n "$TEST_CODEX_ARGS_LOG" ]; then
  if [ -z "$TEST_CODEX_ARGS_LOG_APPEND" ]; then
    : > "$TEST_CODEX_ARGS_LOG"
  fi
  for arg in "$@"; do
    printf '%s\n' "$arg" >> "$TEST_CODEX_ARGS_LOG"
  done
fi
if [ -n "$TEST_CODEX_STDIN_LOG" ]; then
  cat > "$TEST_CODEX_STDIN_LOG"
fi
if [ -n "$TEST_LONG_RUNNING_RUN" ]; then
  sleep "$TEST_LONG_RUNNING_RUN"
fi
if [ "$1" = "login" ]; then
  mkdir -p "$CODEX_HOME"
  with_api_key=0
  for arg in "$@"; do
    if [ "$arg" = "--with-api-key" ]; then
      with_api_key=1
    fi
  done
  if [ "$with_api_key" = "1" ]; then
    IFS= read -r api_key
    printf '{"auth_mode":"apikey","OPENAI_API_KEY":"%s"}\n' "$api_key" > "$CODEX_HOME/auth.json"
    exit 0
  fi
  account_id="${TEST_LOGIN_ACCOUNT_ID:-main-account}"
  token="${TEST_LOGIN_ACCESS_TOKEN:-test-token}"
  id_token="${TEST_LOGIN_ID_TOKEN:-}"
  if [ -n "$id_token" ]; then
    printf '{"tokens":{"id_token":"%s","access_token":"%s","account_id":"%s"}}\n' "$id_token" "$token" "$account_id" > "$CODEX_HOME/auth.json"
  else
    printf '{"tokens":{"access_token":"%s","account_id":"%s"}}\n' "$token" "$account_id" > "$CODEX_HOME/auth.json"
  fi
fi
session_marker="${TEST_SESSION_MARKER:-}"
if [ -n "$session_marker" ]; then
  mkdir -p "$CODEX_HOME/sessions"
  printf '%s\n' "$session_marker" >> "$CODEX_HOME/history.jsonl"
  printf '{"marker":"%s"}\n' "$session_marker" > "$CODEX_HOME/sessions/$session_marker.json"
fi
memory_marker="${TEST_MEMORY_MARKER:-}"
if [ -n "$memory_marker" ]; then
  mkdir -p "$CODEX_HOME/memories"
  printf '{"memory":"%s"}\n' "$memory_marker" > "$CODEX_HOME/memories/$memory_marker.json"
fi
exit 0
"#,
    );

    Fixture {
        _temp_dir: temp_dir,
        usage_server,
        usage_base_url,
        prodex_home,
        shared_codex_home,
        main_home,
        second_home,
        codex_log,
        codex_args_log,
        codex_stdin_log,
        codex_bin,
    }
}

pub(crate) fn write_json(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create json parent dir");
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("failed to encode json"),
    )
    .expect("failed to write json");
}

fn write_executable(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create executable parent dir");
    }
    fs::write(path, content).expect("failed to write executable");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(path, permissions).expect("failed to chmod executable");
    }
}
