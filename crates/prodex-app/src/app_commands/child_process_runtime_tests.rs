use super::*;
use std::fs;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct CompanionFixture {
    root: PathBuf,
}

impl CompanionFixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // Keep AF_UNIX names comfortably below the macOS sun_path limit.
        let root = std::env::temp_dir().join(format!("pc-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self { root }
    }
}

impl Drop for CompanionFixture {
    fn drop(&mut self) {
        // Only the synthetic grandchild created by this fixture may be stopped.
        // This rescue also keeps the pre-patch failing test from leaking a daemon.
        if UnixStream::connect(self.root.join("live")).is_ok()
            && let Ok(pid) = fs::read_to_string(self.root.join("pid"))
            && let Ok(pid) = pid.trim().parse::<libc::pid_t>()
            && pid > 0
        {
            // SAFETY: this PID comes only from the live synthetic child in
            // this fixture's private directory, never a user process registry.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn interactive_companion_teardown_reaps_wrapped_native_server() {
    let _environment = crate::TestEnvVarGuard::set(crate::SUB_AGENT_RECURSION_MARKER, "1");
    assert!(
        !super::super::child_owns_private_process_group(),
        "exercise the same non-private TUI branch used by interactive launch"
    );
    let fixture = CompanionFixture::new();
    let script = fixture.root.join("wrapper.py");
    fs::write(
        &script,
        r#"import os, pathlib, select, socket, subprocess, sys, time
root = pathlib.Path(sys.argv[1])
if len(sys.argv) == 2:
    # Like the official npm launcher, this process wraps a native app-server.
    child = subprocess.Popen([sys.executable, __file__, str(root), 'native'])
    child.wait()
else:
    live = socket.socket(socket.AF_UNIX)
    live.bind(str(root / 'live'))
    live.listen(16)
    (root / 'pid').write_text(str(os.getpid()))
    ready = socket.socket(socket.AF_UNIX)
    ready.bind(str(root / '.s'))
    ready.listen(16)
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        for listener in select.select([live, ready], [], [], 0.1)[0]:
            connection, _ = listener.accept()
            connection.close()
"#,
    )
    .unwrap();
    let home = fixture.root.join("home");
    fs::create_dir(&home).unwrap();
    let child = ChildProcessPlan::new("sh".into(), home.clone())
        .with_args(vec!["-c".into(), "exit 0".into()]);
    let companion = ChildProcessPlan::new("python3".into(), home).with_args(vec![
        script.into_os_string(),
        fixture.root.as_os_str().to_owned(),
    ]);
    let plan =
        RuntimeLaunchPlan::new(child).with_unix_companion(companion, fixture.root.join(".s"));
    let status = run_runtime_launch_plan_unix(&plan, None, None)
        .expect("synthetic companion and TUI must launch");
    assert!(status.success());
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline && UnixStream::connect(fixture.root.join("live")).is_ok() {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        UnixStream::connect(fixture.root.join("live")).is_err(),
        "native app-server outlived its launcher; deleting its overlay/proxy leaves an orphan with broken retries",
    );
}
