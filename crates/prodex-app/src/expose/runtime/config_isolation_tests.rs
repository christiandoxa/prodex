use super::cloudflared::CloudflaredConfigIsolation;
#[cfg(unix)]
use crate::ExposeArgs;
use crate::test_temp_root;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cloudflared_config_isolation_does_not_touch_user_config() {
    let root = test_temp_root().join(format!(
        "prodex-cloudflared-config-fixture-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let user_config = root.join("home/.cloudflared/config.yaml");
    fs::create_dir_all(user_config.parent().unwrap()).unwrap();
    fs::write(
        &user_config,
        "tunnel: named-tunnel\ncredentials-file: secret.json\n",
    )
    .unwrap();

    let config = CloudflaredConfigIsolation::create().unwrap();
    let private_config = config.path.clone();
    assert!(!private_config.starts_with(user_config.parent().unwrap()));
    assert!(private_config.is_file());
    assert_eq!(
        fs::read_to_string(&user_config).unwrap(),
        "tunnel: named-tunnel\ncredentials-file: secret.json\n"
    );
    fs::write(config.directory().join("runtime-cache"), "temporary").unwrap();

    drop(config);
    assert!(!private_config.exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn pty_shutdown_terminates_background_processes_in_its_session() {
    use super::ExposePty;
    use std::time::{Duration, Instant};

    let root = test_temp_root().join(format!(
        "prodex-pty-process-tree-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    let pid_path = root.join("child.pid");
    let pid_path = pid_path.to_string_lossy().replace('\'', "'\\\"'\\\"'");
    let args = ExposeArgs {
        command: Some(format!("sleep 30 & echo $! > '{pid_path}'; wait")),
        cols: 80,
        rows: 24,
        max_clients: 1,
        tunnel: false,
        no_tunnel: false,
        tunnel_provider: None,
        openai_tunnel_id: None,
        name: None,
        invocation: prodex_cli::ExposeInvocation::Standalone,
        super_args: None,
    };
    let pty = ExposePty::spawn(&args).unwrap();
    let pid_path = root.join("child.pid");
    let deadline = Instant::now() + Duration::from_secs(2);
    let child_pid = loop {
        if let Ok(pid) = fs::read_to_string(&pid_path)
            && let Ok(pid) = pid.trim().parse::<libc::pid_t>()
        {
            break pid;
        }
        assert!(
            Instant::now() < deadline,
            "background process did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    };

    pty.shutdown();

    let deadline = Instant::now() + Duration::from_secs(2);
    while unsafe { libc::kill(child_pid, 0) } == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_ne!(unsafe { libc::kill(child_pid, 0) }, 0);
    let _ = fs::remove_dir_all(root);
}
