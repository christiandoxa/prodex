#[cfg(unix)]
use std::io::Read;
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;
#[cfg(unix)]
use std::time::{Duration, Instant};
#[cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt};

#[path = "auto_rotate/support/temp_dir.rs"]
mod temp_dir;
use temp_dir::TestDir;

#[cfg(unix)]
#[test]
fn mcp_jsonl_bridge_reports_child_failure_without_waiting_for_stdin_eof() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args([
            "__mcp-jsonl-bridge",
            "sh",
            "-c",
            "printf 'cohort mismatch: active daemon build differs\\n' >&2; exit 7",
        ])
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
    assert!(
        stderr.contains("cohort mismatch: active daemon build differs"),
        "unexpected stderr: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn mcp_jsonl_bridge_stops_child_when_client_stdin_closes() {
    let started_at = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args(["__mcp-jsonl-bridge", "sh", "-c", "exec sleep 2"])
        .stdin(Stdio::null())
        .output()
        .expect("bridge should run");

    assert!(
        output.status.success(),
        "unexpected bridge failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        started_at.elapsed() < Duration::from_secs(1),
        "bridge should stop the child when client stdin closes"
    );
}

#[cfg(unix)]
#[test]
fn mcp_jsonl_bridge_stops_child_after_malformed_output() {
    let started_at = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args([
            "__mcp-jsonl-bridge",
            "sh",
            "-c",
            "printf 'not-json\\n'; exec sleep 30",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("bridge should run");

    assert!(!output.status.success());
    assert!(
        started_at.elapsed() < Duration::from_secs(3),
        "bridge should stop a child whose output is malformed"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("failed to parse MCP JSON line"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn sub_agent_exec_is_shell_free_preserves_streams_status_and_releases_slot() {
    let temp_dir = TestDir::new();
    let task_dir = temp_dir.path.join("task files");
    let slot_dir = temp_dir.path.join("slots");
    fs::create_dir_all(&task_dir).unwrap();
    fs::create_dir_all(&slot_dir).unwrap();
    fs::write(slot_dir.join("slot-00.lock"), "").unwrap();
    let fake_child = temp_dir.path.join("fake child '引用");
    fs::write(
        &fake_child,
        "#!/bin/sh\nif [ \"$PRODEX_TEST_CHILD_MODE\" = empty ]; then exit 0; fi\nprintf 'MARKER=%s\\n' \"$PRODEX_SUB_AGENT\" > \"$PRODEX_TEST_CAPTURE\"\nprintf 'ARG\\n%s\\n' \"$@\" >> \"$PRODEX_TEST_CAPTURE\"\nprintf 'child stdout\\n'\nprintf 'child stderr\\n' >&2\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&fake_child, fs::Permissions::from_mode(0o700)).unwrap();
    let config = temp_dir.path.join("launcher config.json");
    fs::write(
        &config,
        serde_json::to_vec_pretty(&serde_json::json!({
            "executable": fake_child,
            "provider": "copilot",
            "model": "model exact '引用",
            "effort": "xhigh",
            "local-url": null,
            "presidio-enabled": true,
            "max-concurrency": { "value": 1, "source": "custom" },
            "slot-dir": slot_dir,
            "task-dir": task_dir,
            "task-max-bytes": 65_536,
            "recursion-marker": "PRODEX_SUB_AGENT"
        }))
        .unwrap(),
    )
    .unwrap();
    let capture = temp_dir.path.join("captured argv.txt");
    let task = "spaces 'apostrophe' \"quotes\"\nUnicode 任务; $(printf shell-expanded) & |";

    for name in ["first task.txt", "second task.txt"] {
        let task_file = task_dir.join(name);
        fs::write(&task_file, task).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_prodex"))
            .args(["__sub-agent-exec", "--config"])
            .arg(&config)
            .arg("--task-file")
            .arg(&task_file)
            .env("PATH", "")
            .env("PRODEX_TEST_CAPTURE", &capture)
            .output()
            .expect("internal launcher should run without prodex in PATH");
        assert_eq!(
            output.status.code(),
            Some(7),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "child stdout\n");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("child stderr\n"), "{stderr}");
        assert!(
            stderr.contains("sub-agent child exited with status 7"),
            "{stderr}"
        );
        assert!(!task_file.exists());

        let argv = fs::read_to_string(&capture).unwrap();
        assert!(argv.starts_with("MARKER=1\n"), "{argv}");
        assert_eq!(argv.matches("ARG\n--no-sub-agent\n").count(), 1);
        assert_eq!(argv.matches("ARG\n--presidio\n").count(), 1);
        assert!(!argv.contains("ARG\n--no-presidio\n"));
        assert!(argv.contains("ARG\n--provider\nARG\ncopilot\n"));
        assert!(argv.contains("ARG\n--model\nARG\nmodel exact '引用\n"));
        assert!(argv.contains("ARG\n-c\nARG\nmodel_reasoning_effort=xhigh\n"));
        assert!(argv.contains("ARG\nexec\n"));
        assert_eq!(argv.matches(task).count(), 1, "{argv}");
        assert!(!argv.contains("00000000-0000-7000-8000-000000000042"));
    }

    let empty_task = task_dir.join("empty task.txt");
    fs::write(&empty_task, "empty result task").unwrap();
    let empty_output = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args(["__sub-agent-exec", "--config"])
        .arg(&config)
        .arg("--task-file")
        .arg(&empty_task)
        .env("PRODEX_TEST_CHILD_MODE", "empty")
        .output()
        .expect("empty-result launcher should run");
    assert_eq!(empty_output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&empty_output.stderr)
            .contains("sub-agent child completed without output")
    );
    assert!(!empty_task.exists());

    let recursive_task = task_dir.join("recursive task.txt");
    fs::write(&recursive_task, "recursive result task").unwrap();
    let recursive_output = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args(["__sub-agent-exec", "--config"])
        .arg(&config)
        .arg("--task-file")
        .arg(&recursive_task)
        .env("PRODEX_SUB_AGENT", "1")
        .output()
        .expect("recursive launcher should run");
    assert_eq!(recursive_output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&recursive_output.stderr)
            .contains("hidden sub-agent launcher cannot be invoked recursively")
    );
    assert!(recursive_task.exists());
}

#[cfg(unix)]
#[test]
fn sub_agent_exec_sigterm_reaps_child_and_releases_slot() {
    let temp_dir = TestDir::new();
    let task_dir = temp_dir.path.join("tasks");
    let slot_dir = temp_dir.path.join("slots");
    fs::create_dir_all(&task_dir).unwrap();
    fs::create_dir_all(&slot_dir).unwrap();
    fs::write(slot_dir.join("slot-00.lock"), "").unwrap();
    let fake_child = temp_dir.path.join("blocking child");
    fs::write(
        &fake_child,
        "#!/bin/sh\nif [ \"$PRODEX_TEST_CHILD_MODE\" = quick ]; then printf '%s\\n' 'quick child output'; exit 0; fi\nprintf '%s\\n' \"$$\" > \"$PRODEX_TEST_CHILD_PID_FILE\"\nexec sleep 30\n",
    )
    .unwrap();
    fs::set_permissions(&fake_child, fs::Permissions::from_mode(0o700)).unwrap();
    let config = temp_dir.path.join("config.json");
    fs::write(
        &config,
        serde_json::to_vec(&serde_json::json!({
            "executable": fake_child,
            "provider": "openai",
            "model": null,
            "effort": null,
            "local-url": null,
            "presidio-enabled": false,
            "max-concurrency": { "value": 1, "source": "custom" },
            "slot-dir": slot_dir,
            "task-dir": task_dir,
            "task-max-bytes": 65_536,
            "recursion-marker": "PRODEX_SUB_AGENT"
        }))
        .unwrap(),
    )
    .unwrap();
    let task_file = task_dir.join("first.txt");
    fs::write(&task_file, "narrow task").unwrap();
    let child_pid_file = temp_dir.path.join("child.pid");
    let mut launcher = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args(["__sub-agent-exec", "--config"])
        .arg(&config)
        .arg("--task-file")
        .arg(&task_file)
        .env("PRODEX_TEST_CHILD_PID_FILE", &child_pid_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(3);
    while !child_pid_file.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !child_pid_file.exists() {
        let _ = launcher.kill();
        let _ = launcher.wait();
        panic!("sub-agent child did not become active");
    }
    let child_pid = fs::read_to_string(&child_pid_file)
        .unwrap()
        .trim()
        .parse::<i32>()
        .unwrap();
    assert_eq!(
        unsafe { libc::kill(launcher.id() as i32, libc::SIGTERM) },
        0
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = launcher.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = unsafe { libc::kill(child_pid, libc::SIGKILL) };
            let _ = launcher.kill();
            let _ = launcher.wait();
            panic!("sub-agent launcher ignored SIGTERM");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success());
    assert_eq!(unsafe { libc::kill(child_pid, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );

    let retry_task = task_dir.join("retry.txt");
    fs::write(&retry_task, "retry task").unwrap();
    let retry = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args(["__sub-agent-exec", "--config"])
        .arg(&config)
        .arg("--task-file")
        .arg(&retry_task)
        .env("PRODEX_TEST_CHILD_MODE", "quick")
        .output()
        .unwrap();
    assert!(
        retry.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&retry.stderr)
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
