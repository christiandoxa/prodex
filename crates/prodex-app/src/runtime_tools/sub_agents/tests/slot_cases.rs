use super::*;

#[test]
fn repeated_setup_reconciles_exact_slots_and_protects_active_downsize() {
    let root = temp_test_root("sub-agent-slot-reconcile");
    create_private_directory(&root).unwrap();
    let mut resolved =
        resolve_super_sub_agent_config(SubAgentConfig::default(), SuperLaunchTarget::Fresh)
            .unwrap();
    resolved.max_concurrency = prodex_cli::parse_sub_agent_max_concurrency("8").unwrap();
    let executable = PathBuf::from("/opt/Prodex Binary/prodex");

    write_sub_agent_overlay_with_executable(&root, &resolved, executable.clone()).unwrap();
    write_sub_agent_overlay_with_executable(&root, &resolved, executable.clone()).unwrap();
    let slot_dir = root.join(SUB_AGENT_SLOT_DIR);
    assert_eq!(fs::read_dir(&slot_dir).unwrap().count(), 8);
    let mut names = fs::read_dir(&slot_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        (0..8)
            .map(|index| format!("slot-{index:02}.lock"))
            .collect::<Vec<_>>()
    );

    let active = OpenOptions::new()
        .read(true)
        .write(true)
        .open(slot_dir.join("slot-07.lock"))
        .unwrap();
    FileExt::lock_exclusive(&active).unwrap();
    resolved.max_concurrency = prodex_cli::parse_sub_agent_max_concurrency("4").unwrap();
    let error =
        write_sub_agent_overlay_with_executable(&root, &resolved, executable.clone()).unwrap_err();
    assert!(
        error.to_string().contains("wait for active children"),
        "{error:#}"
    );
    assert_eq!(fs::read_dir(&slot_dir).unwrap().count(), 8);
    FileExt::unlock(&active).unwrap();
    drop(active);

    write_sub_agent_overlay_with_executable(&root, &resolved, executable.clone()).unwrap();
    assert_eq!(fs::read_dir(&slot_dir).unwrap().count(), 4);
    resolved.max_concurrency = prodex_cli::parse_sub_agent_max_concurrency("16").unwrap();
    write_sub_agent_overlay_with_executable(&root, &resolved, executable).unwrap();
    assert_eq!(fs::read_dir(&slot_dir).unwrap().count(), 16);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&slot_dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.join(SUB_AGENT_TASK_DIR))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.join(SUB_AGENT_CONFIG_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn slot_limits_are_bounded_release_and_reusable() {
    for limit in [1, 4, 8, 23, 64] {
        let root = temp_test_root("sub-agent-slot-limit");
        let spec = slot_spec(&root, limit);
        let mut leases = Vec::new();
        let mut maximum_observed_concurrency = 0;
        for _ in 0..limit {
            leases.push(acquire_sub_agent_slot(&spec).unwrap());
            maximum_observed_concurrency = maximum_observed_concurrency.max(leases.len());
        }
        assert!(maximum_observed_concurrency <= usize::from(limit));
        assert_eq!(maximum_observed_concurrency, usize::from(limit));
        assert_eq!(leases.len(), usize::from(limit));
        let error = acquire_sub_agent_slot(&spec).unwrap_err().to_string();
        assert!(
            error.contains("sub-agent concurrency limit reached"),
            "{error}"
        );
        drop(leases.pop());
        leases.push(acquire_sub_agent_slot(&spec).unwrap());
        assert_eq!(leases.len(), usize::from(limit));
        drop(leases);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn failed_spawn_releases_its_cross_process_slot() {
    let root = temp_test_root("sub-agent-failed-spawn");
    let mut spec = slot_spec(&root, 1);
    spec.executable = root.join("missing-prodex-binary");
    let error = handle_sub_agent_exec(exec_args(&root, &spec, "narrow task")).unwrap_err();
    assert!(
        spec.task_dir.join("task.txt").exists()
            && error
                .to_string()
                .contains("failed to spawn sub-agent child")
    );
    drop(acquire_sub_agent_slot(&spec).unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_task_is_rejected_by_mojo_before_slot_admission() {
    let root = temp_test_root("sub-agent-empty-task");
    let spec = slot_spec(&root, 1);
    let args = exec_args(&root, &spec, " \u{3000}\t");
    let error = handle_sub_agent_exec(args).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("sub-agent task must be nonempty")
    );
    assert!(spec.task_dir.join("task.txt").exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn mojo_child_outcome_plan_preserves_failure_precedence() {
    use std::os::unix::process::ExitStatusExt;

    let cancelled = finish_sub_agent_child(SubAgentChildOutcome {
        status: std::process::ExitStatus::from_raw(0),
        cancelled: true,
        output_incomplete: true,
        output_bytes: 0,
    })
    .unwrap_err();
    assert!(cancelled.to_string().contains("launcher cancelled"));
    assert!(cancelled.to_string().contains("output was incomplete"));

    let child_failed = finish_sub_agent_child(SubAgentChildOutcome {
        status: std::process::ExitStatus::from_raw(7 << 8),
        cancelled: false,
        output_incomplete: true,
        output_bytes: 0,
    })
    .unwrap_err();
    assert!(
        child_failed
            .to_string()
            .contains("child exited with status 7")
    );
    assert!(child_failed.to_string().contains("output was incomplete"));

    let no_output = finish_sub_agent_child(SubAgentChildOutcome {
        status: std::process::ExitStatus::from_raw(0),
        cancelled: false,
        output_incomplete: false,
        output_bytes: 0,
    })
    .unwrap_err();
    assert!(no_output.to_string().contains("completed without output"));
}

#[test]
fn inherited_output_pipe_is_bounded_and_cannot_hold_a_slot() {
    let root = temp_test_root("sub-agent-held-output-pipe");
    let spec = slot_spec(&root, 1);
    let started = std::time::Instant::now();
    {
        let _slot = acquire_sub_agent_slot(&spec).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime
            .block_on(async {
                let (stdout_reader, _held_by_descendant) = tokio::io::duplex(1);
                let (stderr_reader, stderr_writer) = tokio::io::duplex(1);
                drop(stderr_writer);
                drain_child_output_tasks(
                    tokio::spawn(relay_child_output(stdout_reader, tokio::io::sink())),
                    tokio::spawn(relay_child_output(stderr_reader, tokio::io::sink())),
                )
                .await
            })
            .unwrap_err();
        assert!(error.to_string().contains("output drain timed out"));
    }
    assert!(started.elapsed() < Duration::from_secs(1));
    drop(acquire_sub_agent_slot(&spec).unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn child_process_group_cleanup_reaches_descendants() {
    let root = temp_test_root("sub-agent-process-group");
    fs::create_dir_all(&root).unwrap();
    let pid_file = root.join("descendant.pid");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut command = tokio::process::Command::new("sh");
        command.args([
            "-c",
            "sleep 30 & echo $! > \"$1\"; wait",
            "sh",
            pid_file.to_str().unwrap(),
        ]);
        configure_sub_agent_child_process_group(&mut command);
        let mut child = command.spawn().unwrap();
        let process_group_id = child.id();
        let mut descendant = None;
        for _ in 0..100 {
            descendant = fs::read_to_string(&pid_file)
                .ok()
                .and_then(|value| value.trim().parse::<libc::pid_t>().ok());
            if descendant.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let descendant = descendant.expect("descendant pid should be written completely");

        assert_eq!(
            unsafe { libc::getpgid(descendant) },
            process_group_id.unwrap() as libc::pid_t
        );
        terminate_sub_agent_child(&mut child, process_group_id).unwrap();
        child.wait().await.unwrap();
        for _ in 0..100 {
            let absent = unsafe { libc::kill(descendant, 0) } != 0
                && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
            let zombie = fs::read_to_string(format!("/proc/{descendant}/stat"))
                .ok()
                .and_then(|stat| stat.rsplit_once(") ").map(|(_, rest)| rest.to_string()))
                .and_then(|rest| rest.split_whitespace().next().map(str::to_string))
                .is_some_and(|state| state == "Z");
            if absent || zombie {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("descendant remained alive after process-group cleanup");
    });
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn child_job_cleanup_reaches_descendants() {
    use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    let root = temp_test_root("sub-agent-job");
    fs::create_dir_all(&root).unwrap();
    let start_file = root.join("start");
    let pid_file = root.join("descendant.pid");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let descendant = runtime.block_on(async {
            let mut command = tokio::process::Command::new("powershell.exe");
            command
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "while (-not (Test-Path -LiteralPath $env:PRODEX_TEST_START_FILE)) { Start-Sleep -Milliseconds 10 }; $child = Start-Process powershell.exe -ArgumentList @('-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 30') -PassThru; Set-Content -LiteralPath $env:PRODEX_TEST_PID_FILE -Value $child.Id",
                ])
                .env("PRODEX_TEST_START_FILE", &start_file)
                .env("PRODEX_TEST_PID_FILE", &pid_file);
            let mut child = command.spawn().unwrap();
            let job = assign_sub_agent_child_job(&child).unwrap();
            fs::write(&start_file, "start").unwrap();
            assert!(child.wait().await.unwrap().success());
            let descendant = fs::read_to_string(&pid_file)
                .unwrap()
                .trim()
                .parse::<u32>()
                .unwrap();
            drop(job);
            descendant
        });

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, descendant) };
        if handle.is_null() {
            break;
        }
        // SAFETY: successful OpenProcess returns an owned process handle.
        let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
        if unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) } != WAIT_TIMEOUT {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "descendant remained alive"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn limit_reached_secures_and_preserves_task_for_retry() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_test_root("sub-agent-secure-task");
    let spec = slot_spec(&root, 1);
    let lease = acquire_sub_agent_slot(&spec).unwrap();
    let args = exec_args(&root, &spec, "narrow task");
    let task_file = args.task_file.clone();
    fs::set_permissions(&args.task_file, fs::Permissions::from_mode(0o666)).unwrap();

    let error = handle_sub_agent_exec(args).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("sub-agent concurrency limit reached")
    );
    assert!(task_file.exists());
    assert_eq!(
        fs::metadata(&task_file).unwrap().permissions().mode() & 0o777,
        0o600
    );

    drop(lease);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn slot_holder_process() {
    let Some(root) = env::var_os("PRODEX_TEST_SUB_AGENT_SLOT_ROOT") else {
        return;
    };
    let limit = env::var("PRODEX_TEST_SUB_AGENT_SLOT_LIMIT")
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let result_dir = PathBuf::from(&root).join("results");
    fs::create_dir_all(&result_dir).unwrap();
    let spec = slot_spec(Path::new(&root), limit);
    let result = result_dir.join(format!("{}.txt", std::process::id()));
    match acquire_sub_agent_slot(&spec) {
        Ok(_lease) => {
            fs::write(&result, "acquired").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(1_500));
        }
        Err(error) => {
            assert!(
                error
                    .to_string()
                    .contains("sub-agent concurrency limit reached")
            );
            fs::write(&result, "rejected").unwrap();
        }
    }
}

#[test]
fn separate_processes_share_limit_and_os_releases_stale_slot() {
    let root = temp_test_root("sub-agent-cross-process");
    let spec = slot_spec(&root, 4);
    let test_name = "runtime_tools::sub_agents::tests::slot_cases::slot_holder_process";
    let executable = env::current_exe().unwrap();
    let mut children = (0..5)
        .map(|_| {
            for _ in 0..20 {
                match std::process::Command::new(&executable)
                    .args(["--exact", test_name, "--nocapture"])
                    .env("PRODEX_TEST_SUB_AGENT_SLOT_ROOT", &root)
                    .env("PRODEX_TEST_SUB_AGENT_SLOT_LIMIT", "4")
                    .spawn()
                {
                    Ok(child) => return child,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(error) => panic!("failed to spawn slot holder: {error}"),
                }
            }
            panic!("test executable remained unavailable while spawning slot holder")
        })
        .collect::<Vec<_>>();
    let result_dir = root.join("results");
    for _ in 0..200 {
        if fs::read_dir(&result_dir)
            .map(|entries| entries.count() == 5)
            .unwrap_or(false)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let results = fs::read_dir(&result_dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let pid = path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .parse::<u32>()
                .unwrap();
            (pid, fs::read_to_string(path).unwrap())
        })
        .collect::<Vec<_>>();
    let maximum_observed_concurrency = results
        .iter()
        .filter(|(_, value)| value == "acquired")
        .count();
    assert!(maximum_observed_concurrency <= 4);
    assert_eq!(maximum_observed_concurrency, 4);
    assert_eq!(
        results
            .iter()
            .filter(|(_, value)| value == "rejected")
            .count(),
        1
    );
    let started = std::time::Instant::now();
    let error = acquire_sub_agent_slot(&spec).unwrap_err().to_string();
    assert!(started.elapsed() < std::time::Duration::from_millis(250));
    assert!(error.contains("sub-agent concurrency limit reached"));

    let acquired_pid = results
        .iter()
        .find_map(|(pid, value)| (value == "acquired").then_some(*pid))
        .unwrap();
    let acquired_index = children
        .iter()
        .position(|child| child.id() == acquired_pid)
        .unwrap();
    children[acquired_index].kill().unwrap();
    children[acquired_index].wait().unwrap();
    let lease = acquire_sub_agent_slot(&spec).unwrap();
    drop(lease);
    for (index, child) in children.iter_mut().enumerate() {
        if index != acquired_index {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    fs::remove_dir_all(root).unwrap();
}
