use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        let unique = format!(
            "prodex-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("failed to create temp dir");
        Self { path }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Fixture {
    _temp_dir: TestDir,
    prodex_home: PathBuf,
    cargo_target_dir: PathBuf,
    main_home: PathBuf,
    second_home: PathBuf,
    codex_log: PathBuf,
    codex_bin: PathBuf,
    cq_bin: PathBuf,
}

fn setup_fixture() -> Fixture {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex-home");
    let cargo_target_dir = temp_dir.path.join("cargo-target");
    let homes_root = temp_dir.path.join("homes");
    let bin_root = temp_dir.path.join("bin");
    let main_home = homes_root.join("main");
    let second_home = homes_root.join("second");
    let codex_log = temp_dir.path.join("codex-home.log");
    let codex_bin = bin_root.join("codex");
    let cq_bin = bin_root.join("cq");

    fs::create_dir_all(&prodex_home).expect("failed to create prodex home");
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

    let auth = json!({
        "tokens": {
            "access_token": "test-token"
        }
    });
    write_json(&main_home.join("auth.json"), &auth);
    write_json(&second_home.join("auth.json"), &auth);

    write_executable(
        &codex_bin,
        r#"#!/bin/sh
printf '%s\n' "$CODEX_HOME" > "$TEST_CODEX_LOG"
exit 0
"#,
    );

    write_executable(
        &cq_bin,
        r#"#!/bin/sh
profile="$(basename "$CODEX_HOME")"
if [ "$profile" = "main" ]; then
  printf '%s\n' '{"rate_limit":{"primary_window":{"used_percent":100,"reset_at":1700000000,"limit_window_seconds":18000},"secondary_window":{"used_percent":20,"reset_at":1700000000,"limit_window_seconds":604800}}}'
else
  printf '%s\n' '{"rate_limit":{"primary_window":{"used_percent":20,"reset_at":1700000000,"limit_window_seconds":18000},"secondary_window":{"used_percent":30,"reset_at":1700000000,"limit_window_seconds":604800}}}'
fi
exit 0
"#,
    );

    Fixture {
        _temp_dir: temp_dir,
        prodex_home,
        cargo_target_dir,
        main_home,
        second_home,
        codex_log,
        codex_bin,
        cq_bin,
    }
}

fn write_json(path: &Path, value: &Value) {
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

fn run_prodex(fixture: &Fixture, args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_TARGET_DIR", &fixture.cargo_target_dir)
        .env("PRODEX_HOME", &fixture.prodex_home)
        .env("PRODEX_CODEX_BIN", &fixture.codex_bin)
        .env("PRODEX_CQ_BIN", &fixture.cq_bin)
        .env("TEST_CODEX_LOG", &fixture.codex_log)
        .args(["run", "--quiet", "--bin", "prodex", "--"])
        .args(args)
        .output()
        .expect("failed to execute prodex")
}

fn active_profile(path: &Path) -> String {
    let state: Value = serde_json::from_slice(
        &fs::read(path.join("state.json")).expect("failed to read state.json"),
    )
    .expect("failed to parse state.json");

    state["active_profile"]
        .as_str()
        .expect("active_profile should be a string")
        .to_string()
}

#[test]
fn run_auto_rotates_active_profile_when_current_is_blocked() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Auto-rotating to profile 'second'."));
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
}

#[test]
fn explicit_profile_auto_rotates_by_default() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run", "--profile", "main"]);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("Auto-rotating to profile 'second'."));
    assert_eq!(active_profile(&fixture.prodex_home), "second");
    assert_eq!(
        fs::read_to_string(&fixture.codex_log)
            .expect("failed to read codex log")
            .trim(),
        fixture.second_home.display().to_string()
    );
}

#[test]
fn explicit_profile_can_disable_auto_rotate() {
    let fixture = setup_fixture();

    let output = run_prodex(&fixture, &["run", "--profile", "main", "--no-auto-rotate"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Other profiles that look ready: second")
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Rerun without `--no-auto-rotate` to allow fallback.")
    );
    assert_eq!(active_profile(&fixture.prodex_home), "main");
    assert!(!fixture.codex_log.exists());
    assert_eq!(
        fixture.main_home.file_name().and_then(|name| name.to_str()),
        Some("main")
    );
}
