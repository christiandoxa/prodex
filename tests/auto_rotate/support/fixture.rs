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
    let bin_root = temp_dir.path.join("bin");
    let main_home = prodex_home.join("profiles/main");
    let second_home = prodex_home.join("profiles/second");
    let codex_log = temp_dir.path.join("codex-home.log");
    let codex_args_log = temp_dir.path.join("codex-args.log");
    let codex_stdin_log = temp_dir.path.join("codex-stdin.log");
    let codex_bin = bin_root.join(if cfg!(windows) { "codex.cmd" } else { "codex" });

    secure_test_directory(&temp_dir.path);
    fs::create_dir_all(&shared_codex_home).expect("failed to create shared codex home");
    fs::create_dir_all(&bin_root).expect("failed to create bin dir");
    for directory in [
        &prodex_home,
        &prodex_home.join("profiles"),
        &main_home,
        &second_home,
    ] {
        secure_test_directory(directory);
    }

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

    #[cfg(not(windows))]
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
    chmod 600 "$CODEX_HOME/auth.json"
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
  chmod 600 "$CODEX_HOME/auth.json"
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

    #[cfg(windows)]
    write_executable(
        &codex_bin,
        r#"@echo off
setlocal EnableExtensions
> "%TEST_CODEX_LOG%" echo %CODEX_HOME%
if defined TEST_CODEX_LOG_APPEND >> "%TEST_CODEX_LOG_APPEND%" echo %CODEX_HOME%
set "prodex_first_arg=%~1"
set "prodex_with_api_key="
if defined TEST_CODEX_ARGS_LOG if not defined TEST_CODEX_ARGS_LOG_APPEND type nul > "%TEST_CODEX_ARGS_LOG%"
:prodex_args
if "%~1"=="" goto prodex_stdin
if defined TEST_CODEX_ARGS_LOG >> "%TEST_CODEX_ARGS_LOG%" echo %~1
if /I "%~1"=="--with-api-key" set "prodex_with_api_key=1"
shift
goto prodex_args
:prodex_stdin
if defined TEST_CODEX_STDIN_LOG more > "%TEST_CODEX_STDIN_LOG%"
if defined TEST_LONG_RUNNING_RUN powershell.exe -NoProfile -NonInteractive -Command "Start-Sleep -Seconds %TEST_LONG_RUNNING_RUN%"
if /I "%prodex_first_arg%"=="login" goto prodex_login
goto prodex_session
:prodex_login
if not exist "%CODEX_HOME%" mkdir "%CODEX_HOME%"
if defined prodex_with_api_key goto prodex_api_key
if not defined TEST_LOGIN_ACCOUNT_ID set "TEST_LOGIN_ACCOUNT_ID=main-account"
if not defined TEST_LOGIN_ACCESS_TOKEN set "TEST_LOGIN_ACCESS_TOKEN=test-token"
if defined TEST_LOGIN_ID_TOKEN goto prodex_id_token
> "%CODEX_HOME%\auth.json" echo {"tokens":{"access_token":"%TEST_LOGIN_ACCESS_TOKEN%","account_id":"%TEST_LOGIN_ACCOUNT_ID%"}}
goto prodex_session
:prodex_id_token
> "%CODEX_HOME%\auth.json" echo {"tokens":{"id_token":"%TEST_LOGIN_ID_TOKEN%","access_token":"%TEST_LOGIN_ACCESS_TOKEN%","account_id":"%TEST_LOGIN_ACCOUNT_ID%"}}
goto prodex_session
:prodex_api_key
set /p "prodex_api_key="
> "%CODEX_HOME%\auth.json" echo {"auth_mode":"apikey","OPENAI_API_KEY":"%prodex_api_key%"}
exit /b 0
:prodex_session
if not defined TEST_SESSION_MARKER goto prodex_memory
if not exist "%CODEX_HOME%\sessions" mkdir "%CODEX_HOME%\sessions"
>> "%CODEX_HOME%\history.jsonl" echo %TEST_SESSION_MARKER%
> "%CODEX_HOME%\sessions\%TEST_SESSION_MARKER%.json" echo {"marker":"%TEST_SESSION_MARKER%"}
:prodex_memory
if not defined TEST_MEMORY_MARKER goto prodex_done
if not exist "%CODEX_HOME%\memories" mkdir "%CODEX_HOME%\memories"
> "%CODEX_HOME%\memories\%TEST_MEMORY_MARKER%.json" echo {"memory":"%TEST_MEMORY_MARKER%"}
:prodex_done
exit /b 0
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
        if path.file_name().is_some_and(|name| name == "auth.json") {
            secure_test_directory(parent);
        }
    }
    if path.file_name().is_some_and(|name| name == "auth.json") {
        secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
            .write_text(
                &secret_store::SecretLocation::file(path),
                serde_json::to_string_pretty(value).expect("failed to encode auth json"),
            )
            .expect("failed to write auth json securely");
        return;
    }
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("failed to encode json"),
    )
    .expect("failed to write json");
}

fn secure_test_directory(path: &Path) {
    secret_store::ensure_private_directory(path).expect("failed to secure test directory");
}

fn write_executable(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create executable parent dir");
    }
    #[cfg(windows)]
    let content = content.replace('\n', "\r\n");
    fs::write(path, content).expect("failed to write executable");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = fs::Permissions::from_mode(0o755);
        fs::set_permissions(path, permissions).expect("failed to chmod executable");
    }
}
