use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[path = "auto_rotate/support/temp_dir.rs"]
mod temp_dir;
use temp_dir::TestDir;

#[cfg(unix)]
#[test]
fn mcp_jsonl_bridge_reports_child_failure_without_waiting_for_stdin_eof() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args(["__mcp-jsonl-bridge", "sh", "-c", "exit 7"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bridge should start");
    let held_stdin = child.stdin.take().expect("bridge stdin should be piped");
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait().expect("bridge status should be readable") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("bridge waited for client stdin after its child exited");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("bridge stderr should be piped")
        .read_to_string(&mut stderr)
        .expect("bridge stderr should be readable");
    drop(held_stdin);

    assert!(!status.success());
    assert!(
        stderr.contains("MCP server exited with status"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn runtime_goal_notify_rejects_an_invalid_marker_path() {
    let temp_dir = TestDir::new();
    let prodex_home = temp_dir.path.join("prodex-home");
    let marker_path = temp_dir.path.join("outside-monitor.marker");
    let output = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .arg("__runtime-goal-session-notify")
        .arg(&marker_path)
        .arg(r#"{"session_id":"00000000-0000-4000-8000-000000000001"}"#)
        .env("PRODEX_HOME", &prodex_home)
        .output()
        .expect("runtime goal notify hook should run");

    assert!(!output.status.success());
    assert!(!marker_path.exists());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("invalid runtime goal session marker path"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
