use super::*;
use std::fs::{self, File};
use std::process::Command;

fn limits() -> libc::rlimit {
    let mut value = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, value.as_mut_ptr()) },
        0
    );
    unsafe { value.assume_init() }
}

#[test]
fn descriptor_headroom_recovers_state_reads_under_inherited_low_limit() {
    const CHILD: &str = "PRODEX_DESCRIPTOR_HEADROOM_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "process_resources::tests::descriptor_headroom_recovers_state_reads_under_inherited_low_limit", "--nocapture"])
            .env(CHILD, "1")
            .output().expect("isolated resource-limit child must run");
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("DESCRIPTOR_STATE_READ_RECOVERED")
        );
        return;
    }
    let original = limits();
    let hard = original.rlim_max.min(256);
    assert!(
        hard >= 128,
        "descriptor regression needs a child hard limit >=128"
    );
    let root =
        std::env::temp_dir().join(format!("prodex-fd-state-regression-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let state = root.join("state.json");
    let content = b"{\"active_profile\":\"synthetic-owner\"}";
    fs::write(&state, content).unwrap();
    let low = libc::rlimit {
        rlim_cur: 64,
        rlim_max: hard,
    };
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &low) }, 0);
    let mut held = Vec::new();
    loop {
        match File::open("/dev/null") {
            Ok(file) => held.push(file),
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(libc::EMFILE));
                break;
            }
        }
    }
    assert_eq!(
        fs::read(&state).unwrap_err().raw_os_error(),
        Some(libc::EMFILE)
    );
    prepare_descriptor_headroom().expect("soft limit can be raised without opening files");
    let after = limits();
    assert_eq!(after.rlim_max, hard, "must not increase the hard limit");
    assert_eq!(after.rlim_cur, hard);
    assert_eq!(fs::read(&state).unwrap(), content);
    assert!(
        !state.with_extension("json.last-good").exists(),
        "recovery must not rewrite backups"
    );
    drop(held);
    fs::remove_dir_all(root).unwrap();
    println!("DESCRIPTOR_STATE_READ_RECOVERED");
}

#[test]
fn descriptor_headroom_is_inherited_by_exec_child() {
    const CHILD: &str = "PRODEX_DESCRIPTOR_EXEC_TEST_CHILD";
    const TEST: &str = "process_resources::tests::descriptor_headroom_is_inherited_by_exec_child";
    let mode = std::env::var(CHILD).ok();
    if mode.as_deref() == Some("observe") {
        let inherited = limits();
        assert_eq!(inherited.rlim_cur, inherited.rlim_max);
        assert_eq!(inherited.rlim_max, 256);
        println!("DESCRIPTOR_EXEC_CHILD_INHERITED");
        return;
    }
    let child_mode = if mode.as_deref() == Some("prepare") {
        let inherited = limits();
        assert!(
            inherited.rlim_max >= 256,
            "test needs a hard limit of at least 256"
        );
        let low = libc::rlimit {
            rlim_cur: 64,
            rlim_max: 256,
        };
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &low) }, 0);
        prepare_descriptor_headroom().unwrap();
        "observe"
    } else {
        "prepare"
    };
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture"])
        .env(CHILD, child_mode)
        .output()
        .expect("isolated inherited-limit test process must run");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("DESCRIPTOR_EXEC_CHILD_INHERITED"));
    println!("DESCRIPTOR_EXEC_CHILD_INHERITED");
}

#[test]
fn descriptor_headroom_never_lowers_existing_limits() {
    const CHILD: &str = "PRODEX_DESCRIPTOR_PRESERVE_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_resources::tests::descriptor_headroom_never_lowers_existing_limits",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("DESCRIPTOR_LIMIT_PRESERVED"));
        return;
    }
    let before = limits();
    prepare_descriptor_headroom().unwrap();
    let after = limits();
    assert!(after.rlim_cur >= before.rlim_cur);
    assert_eq!(after.rlim_max, before.rlim_max);
    prepare_descriptor_headroom().unwrap();
    assert_eq!(
        limits().rlim_cur,
        after.rlim_cur,
        "bootstrap must be idempotent"
    );
    println!("DESCRIPTOR_LIMIT_PRESERVED");
}
