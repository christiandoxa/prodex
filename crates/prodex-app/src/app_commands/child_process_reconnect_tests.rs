use super::*;
use std::process::{Child, Command};

struct ReconnectFixture {
    fixture: CompanionFixture,
    peer: Child,
}

impl Drop for ReconnectFixture {
    fn drop(&mut self) {
        // Pre-fix reproduction must not leak its synthetic native frontend.
        if UnixStream::connect(self.fixture.root.join("tui-live")).is_ok()
            && let Ok(pid) = fs::read_to_string(self.fixture.root.join("tui-pid"))
            && let Ok(pid) = pid.trim().parse::<libc::pid_t>()
            && pid > 0
        {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
        let _ = self.peer.kill();
        let _ = self.peer.wait();
    }
}

#[test]
fn reconnect_recovery_stops_wrapped_tui_before_removing_its_endpoint() {
    assert_wrapped_tui_shutdown(false);
}

#[test]
fn reconnect_monitor_error_still_reaps_the_native_tui_before_endpoint_cleanup() {
    assert_wrapped_tui_shutdown(true);
}

fn assert_wrapped_tui_shutdown(monitor_error: bool) {
    let _env = crate::TestEnvVarGuard::set(crate::SUB_AGENT_RECURSION_MARKER, "1");
    assert!(!super::super::super::child_owns_private_process_group());
    let fixture = CompanionFixture::new();
    let peer = Command::new("sleep").arg("30").spawn().unwrap();
    let mut owned = ReconnectFixture { fixture, peer };
    let root = &owned.fixture.root;
    let script = root.join("launcher.py");
    fs::write(
        &script,
        r#"import os, pathlib, select, signal, socket, subprocess, sys, time
root = pathlib.Path(sys.argv[1])
mode = sys.argv[2]
if mode in ('server', 'tui'):
    child = subprocess.Popen([sys.executable, __file__, str(root), 'native-' + mode])
    # Mirrors the installed official npm launcher: SIGTERM is forwarded, but
    # SIGKILL cannot run a handler and leaves the native frontend behind.
    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, lambda signum, frame: child.send_signal(signum))
    sys.exit(child.wait())
if mode == 'native-server':
    ready = socket.socket(socket.AF_UNIX)
    ready.bind(str(root / '.s'))
    ready.listen(16)
    live = socket.socket(socket.AF_UNIX)
    live.bind(str(root / 'live'))
    live.listen(16)
    (root / 'pid').write_text(str(os.getpid()))
    while True:
        for listener in select.select([ready, live], [], [], .05)[0]:
            client, _ = listener.accept()
            client.close()
if mode == 'native-tui':
    def stop(signum, frame):
        # Teardown ordering matters: the frontend exits before its endpoint.
        probe = socket.socket(socket.AF_UNIX)
        probe.connect(str(root / '.s'))
        probe.close()
        (root / 'graceful-stop').write_text(str(signum))
        sys.exit(23)
    signal.signal(signal.SIGTERM, stop)
    live = socket.socket(socket.AF_UNIX)
    live.bind(str(root / 'tui-live'))
    live.listen(16)
    (root / 'tui-pid').write_text(str(os.getpid()))
    (root / 'effect-once').write_text('completed tool effect\n')
    (root / 'ready').write_text('ready')
    while True:
        for listener in select.select([live], [], [], .05)[0]:
            client, _ = listener.accept()
            client.close()
"#,
    )
    .unwrap();
    let home = root.join("home");
    fs::create_dir(&home).unwrap();
    let make = |mode: &str| {
        ChildProcessPlan::new("python3".into(), home.clone()).with_args(vec![
            script.as_os_str().to_owned(),
            root.as_os_str().to_owned(),
            mode.into(),
        ])
    };
    let plan =
        RuntimeLaunchPlan::new(make("tui")).with_unix_companion(make("server"), root.join(".s"));
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut requested = false;
    let result = run_runtime_launch_plan_unix(
        &plan,
        None,
        Some(&mut || {
            if root.join("ready").is_file() {
                requested = true;
                anyhow::ensure!(!monitor_error, "synthetic monitor failure");
                return Ok(true);
            }
            anyhow::ensure!(Instant::now() < deadline, "mock TUI did not become ready");
            Ok(false)
        }),
    );
    assert!(requested);
    assert!(
        owned.peer.try_wait().unwrap().is_none(),
        "unrelated terminal peer must survive"
    );
    // A killed process-group grandchild is reaped by the OS, not Child::wait
    // on the launcher. Observe bounded socket closure as the existing companion
    // teardown test does; do not assume signal delivery is synchronous.
    let closed_by = Instant::now() + Duration::from_secs(1);
    while Instant::now() < closed_by && UnixStream::connect(root.join("live")).is_ok() {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        UnixStream::connect(root.join("live")).is_err(),
        "companion must be reaped"
    );
    assert!(
        UnixStream::connect(root.join("tui-live")).is_err(),
        "old native TUI survived its npm-style wrapper and will reconnect to a deleted endpoint"
    );
    if monitor_error {
        let error = result.unwrap_err();
        assert!(
            format!("{error:#}").contains("synthetic monitor failure"),
            "the original monitor error must remain in the contextual error chain"
        );
    } else {
        assert_eq!(
            result.unwrap().code(),
            Some(23),
            "observe the native frontend's graceful exit"
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("graceful-stop")).unwrap(),
        libc::SIGTERM.to_string()
    );
    assert_eq!(
        fs::read_to_string(root.join("effect-once")).unwrap(),
        "completed tool effect\n"
    );
}

#[test]
fn reconnect_graceful_stop_retains_bounded_escalation_for_unresponsive_child() {
    let fixture = CompanionFixture::new();
    let ready = fixture.root.join("ignoring-term");
    let mut command = Command::new("sh");
    command
        .args([
            "-c",
            "trap '' TERM; echo ready > \"$1\"; exec sleep 30",
            "sh",
        ])
        .arg(&ready);
    super::super::super::configure_child_process_group(&mut command, true);
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready.is_file() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if !ready.is_file() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("synthetic signal-ignoring child did not start");
    }
    let started = Instant::now();
    let status = super::super::super::wait_for_child_termination(&mut child, false).unwrap();
    assert!(!status.success());
    assert!(
        started.elapsed() >= Duration::from_secs(2),
        "graceful stop skipped the deadline"
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "force-kill escalation must stay bounded"
    );
}
