use super::run_manager::{
    EXPOSE_MAX_ACTIVE_RUNS, EXPOSE_MAX_QUEUED_RUNS, EXPOSE_MAX_RUN_EVENTS, EXPOSE_MAX_RUN_ID_BYTES,
    EXPOSE_MAX_RUN_OUTPUT_BYTES, ExposeRunManager, ExposeRunResult, ExposeRunState, bounded_text,
    expose_run_id, redacted_output_text,
};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

fn test_super_args() -> prodex_cli::SuperArgs {
    let crate::Commands::Super(mut args) =
        crate::parse_cli_command_from(["prodex", "s", "--no-sub-agent"])
            .expect("test Super args should parse")
    else {
        panic!("expected Super args");
    };
    args.url = Some("http://127.0.0.1:11434/v1".to_string());
    args.local_model = Some("model-a".to_string());
    args.codex_args = vec!["-c".into(), "model_reasoning_effort=max".into()];
    args.sub_agent = true;
    args.no_sub_agent = false;
    args.sub_agent_provider = Some(prodex_provider_core::ProviderId::OpenAi);
    args.sub_agent_model = Some("sub-model".to_string());
    args.sub_agent_model_reasoning_effort = Some(prodex_cli::SubAgentReasoningEffort::High);
    args
}

fn test_root(name: &str) -> PathBuf {
    let root = crate::test_support::test_temp_root()
        .join(format!("prodex-expose-{name}-{}", std::process::id()));
    fs::create_dir_all(&root).expect("test root should be created");
    root
}

fn fake_super(root: &Path) -> PathBuf {
    crate::test_support::write_test_python_executable(
        root,
        "fake-super",
        r#"#!/usr/bin/env python3
import os
import socket
import subprocess
import sys
import time

task = sys.stdin.read()
if task == "sleep":
    child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
    print("CHILD=" + str(child.pid), flush=True)
    time.sleep(30)
elif task == "large":
    print("LARGE=" + ("x" * 300000), flush=True)
elif task == "fail":
    print("FAILED_TASK", flush=True)
    sys.exit(7)
elif task.startswith("outside|"):
    target = task.split("|", 1)[1]
    os.chdir(target)
    with open("created-by-expose-super", "w", encoding="utf-8") as output:
        output.write("ok")
    print("OUTSIDE=" + os.getcwd(), flush=True)
elif task.startswith("exec|"):
    executable = task.split("|", 1)[1]
    command = [executable]
    if os.name == "nt" and executable.lower().endswith((".cmd", ".bat")):
        command = ["cmd", "/c", executable]
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    print("EXEC=" + result.stdout.strip(), flush=True)
elif task.startswith("network|"):
    with socket.create_connection(("127.0.0.1", int(task.split("|", 1)[1])), timeout=2) as client:
        client.sendall(b"ping")
        print("NETWORK=" + client.recv(16).decode("ascii"), flush=True)
elif task.startswith("git|"):
    repository = task.split("|", 1)[1]
    subprocess.run(["git", "-C", repository, "add", "tracked.txt"], check=True)
    subprocess.run([
        "git", "-C", repository, "-c", "user.name=Expose Test",
        "-c", "user.email=expose@example.com", "commit", "-m", "expose test",
    ], check=True, capture_output=True, text=True)
    print("GIT=committed", flush=True)
else:
    print("TASK=" + task, flush=True)
    print("CWD=" + os.getcwd(), flush=True)
    print("ARGV=" + repr(sys.argv), flush=True)
    for sentinel in ("A_ONLY", "B_ONLY", "C_ONLY"):
        if os.path.exists(sentinel):
            print("SENTINEL=" + sentinel, flush=True)
"#,
    )
}

fn wait_for_terminal(manager: &ExposeRunManager, run_id: &str) -> ExposeRunResult {
    let attempts = 1_000;
    for _ in 0..attempts {
        if let Some(result) = manager.result(run_id)
            && result.summary.state.terminal()
        {
            return result;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run {run_id} did not finish")
}

#[test]
fn run_ids_are_opaque_random_and_bounded() {
    let first = expose_run_id().unwrap();
    let second = expose_run_id().unwrap();
    assert_ne!(first, second);
    assert!(first.starts_with("spr_"));
    assert!(first.len() <= EXPOSE_MAX_RUN_ID_BYTES);
}

#[test]
fn output_is_redacted_and_bounded() {
    let output = redacted_output_text(b"Authorization: Bearer secret-value\n\xE7\x95\x8C");
    assert!(!output.contains("secret-value"));
    assert!(bounded_text(&"界".repeat(100), 5).len() <= 5);
}

#[test]
fn run_manager_uses_private_stdin_and_captured_workspace() {
    let root = test_root("stdin");
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let executable = fake_super(&root);
    let manager = ExposeRunManager::new_with_executable(
        workspace.clone(),
        "pdxi_stdin".to_string(),
        "workspace".to_string(),
        executable,
    );
    let run = manager
        .start("TASK_NOT_IN_ARGV".to_string(), test_super_args())
        .unwrap();
    let result = wait_for_terminal(&manager, &run.run_id);
    assert_eq!(result.summary.state, ExposeRunState::Succeeded);
    assert!(result.output.contains("TASK=TASK_NOT_IN_ARGV"));
    assert!(
        result
            .output
            .contains(&format!("CWD={}", workspace.display()))
    );
    assert!(result.output.contains("ARGV="));
    assert!(result.output.contains("--full-access"));
    assert!(result.output.contains("--model"));
    assert!(result.output.contains("model-a"));
    assert!(result.output.contains("model_reasoning_effort=max"));
    assert!(result.output.contains("--sub-agent-model"));
    assert!(result.output.contains("sub-model"));
    assert!(!result.output.contains("TASK_NOT_IN_ARGV'"));
    manager.shutdown();
}

#[test]
fn expose_super_run_retains_os_user_authority_outside_initial_workspace() {
    let root = test_root("authority");
    let workspace = root.join("workspace");
    let outside = root.join("outside");
    let repository = outside.join("repository");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&repository).unwrap();

    let arbitrary = crate::test_support::write_test_python_executable(
        &root,
        "expose-unlisted-tool",
        "#!/usr/bin/env python3\nprint('ARBITRARY_TOOL')\n",
    );
    let git_init = Command::new("git")
        .args(["init", "--quiet", repository.to_str().unwrap()])
        .status()
        .expect("git should be installed for the authority fixture");
    assert!(git_init.success());
    fs::write(repository.join("tracked.txt"), "fixture").unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4];
        stream.read_exact(&mut request).unwrap();
        assert_eq!(&request, b"ping");
        stream.write_all(b"pong").unwrap();
    });

    let manager = ExposeRunManager::new_with_executable(
        workspace.clone(),
        "pdxi_authority".to_string(),
        "authority".to_string(),
        fake_super(&root),
    );
    let outside_run = manager
        .start(format!("outside|{}", outside.display()), test_super_args())
        .unwrap();
    let outside_result = wait_for_terminal(&manager, &outside_run.run_id);
    assert_eq!(outside_result.summary.state, ExposeRunState::Succeeded);
    assert!(outside_result.output.contains("OUTSIDE="));
    assert!(outside.join("created-by-expose-super").is_file());

    let executable_run = manager
        .start(format!("exec|{}", arbitrary.display()), test_super_args())
        .unwrap();
    let executable_result = wait_for_terminal(&manager, &executable_run.run_id);
    assert_eq!(executable_result.summary.state, ExposeRunState::Succeeded);
    assert!(executable_result.output.contains("EXEC=ARBITRARY_TOOL"));

    let network_run = manager
        .start(format!("network|{port}"), test_super_args())
        .unwrap();
    let network_result = wait_for_terminal(&manager, &network_run.run_id);
    assert_eq!(network_result.summary.state, ExposeRunState::Succeeded);
    assert!(network_result.output.contains("NETWORK=pong"));
    server.join().unwrap();

    let git_run = manager
        .start(format!("git|{}", repository.display()), test_super_args())
        .unwrap();
    let git_result = wait_for_terminal(&manager, &git_run.run_id);
    assert_eq!(git_result.summary.state, ExposeRunState::Succeeded);
    assert!(git_result.output.contains("GIT=committed"));
    manager.shutdown();
}

#[test]
fn native_frontend_child_does_not_receive_codex_exec_or_feature_args() {
    let root = test_root("native-argv");
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let executable = fake_super(&root);
    let mut args = test_super_args();
    args.cli = Some(prodex_cli::SuperCliAgent::Gemini);
    args.provider = Some(prodex_cli::SuperExternalProvider::Gemini);
    args.codex_args.clear();
    args.codex_features.current_time_reminder = true;
    args.sub_agent = false;
    args.no_sub_agent = true;
    let manager = ExposeRunManager::new_with_executable(
        workspace,
        "pdxi_native_argv".to_string(),
        "native-argv".to_string(),
        executable,
    );
    let run = manager.start("TASK_NATIVE_ARGV".to_string(), args).unwrap();
    let result = wait_for_terminal(&manager, &run.run_id);
    assert_eq!(result.summary.state, ExposeRunState::Succeeded);
    assert!(result.output.contains("'--cli', 'gemini'"));
    assert!(!result.output.contains("'exec', '-'"));
    assert!(!result.output.contains("current-time-reminder"));
    manager.shutdown();
}

#[test]
fn run_manager_reports_failed_child_without_losing_metadata() {
    let root = test_root("failed");
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let manager = ExposeRunManager::new_with_executable(
        workspace,
        "pdxi_failed".to_string(),
        "failed".to_string(),
        fake_super(&root),
    );
    let run = manager
        .start("fail".to_string(), test_super_args())
        .unwrap();
    let result = wait_for_terminal(&manager, &run.run_id);
    assert_eq!(result.summary.state, ExposeRunState::Failed);
    assert_eq!(result.summary.exit_status, Some(7));
    assert!(result.output.contains("FAILED_TASK"));
    manager.shutdown();
}

#[test]
fn run_manager_bounds_output_and_rejects_an_overfull_queue() {
    let root = test_root("bounds");
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let executable = fake_super(&root);
    let manager = ExposeRunManager::new_with_executable(
        workspace,
        "pdxi_bounds".to_string(),
        "bounds".to_string(),
        executable,
    );
    let large = manager
        .start("large".to_string(), test_super_args())
        .unwrap();
    let result = wait_for_terminal(&manager, &large.run_id);
    assert!(result.output_truncated);
    assert!(result.output.len() <= EXPOSE_MAX_RUN_OUTPUT_BYTES);
    assert!(
        manager
            .events(&large.run_id, 0, EXPOSE_MAX_RUN_EVENTS)
            .unwrap()
            .events
            .iter()
            .any(|event| event.event_type == "output_truncated")
    );

    for _ in 0..(EXPOSE_MAX_ACTIVE_RUNS + EXPOSE_MAX_QUEUED_RUNS) {
        manager
            .start("sleep".to_string(), test_super_args())
            .expect("bounded queue should accept its capacity");
    }
    assert!(matches!(
        manager.start("sleep".to_string(), test_super_args()),
        Err("run queue is full")
    ));
    manager.shutdown();
}

#[test]
fn three_run_managers_execute_concurrently_without_output_crossing() {
    let root = test_root("parallel");
    let executable = fake_super(&root);
    let mut managers = Vec::new();
    let mut runs = Vec::new();
    for label in ["A", "B", "C"] {
        let workspace = root.join(format!("workspace-{label}"));
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join(format!("{label}_ONLY")), label).unwrap();
        let manager = ExposeRunManager::new_with_executable(
            workspace,
            format!("pdxi_{label}"),
            label.to_string(),
            executable.clone(),
        );
        let run = manager
            .start(format!("TASK_{label}"), test_super_args())
            .unwrap();
        managers.push(manager);
        runs.push(run.run_id);
    }

    assert!(managers[0].status(&runs[1]).is_none());
    assert!(managers[0].events(&runs[1], 0, 10).is_none());
    assert!(managers[0].result(&runs[1]).is_none());
    assert!(managers[0].cancel(&runs[1]).is_none());

    for (index, label) in ["A", "B", "C"].into_iter().enumerate() {
        let result = wait_for_terminal(&managers[index], &runs[index]);
        assert_eq!(result.summary.state, ExposeRunState::Succeeded);
        assert!(result.output.contains(&format!("TASK=TASK_{label}")));
        assert!(result.output.contains(&format!("SENTINEL={label}_ONLY")));
        for other in ["A", "B", "C"] {
            if other != label {
                assert!(!result.output.contains(&format!("SENTINEL={other}_ONLY")));
            }
        }
    }
    for manager in &managers {
        manager.shutdown();
    }
}

#[test]
fn cancelling_one_run_kills_only_its_process_tree() {
    let root = test_root("cancel");
    let executable = fake_super(&root);
    let mut managers = Vec::new();
    let mut runs = Vec::new();
    for label in ["A", "B", "C"] {
        let workspace = root.join(format!("workspace-{label}"));
        fs::create_dir_all(&workspace).unwrap();
        let manager = ExposeRunManager::new_with_executable(
            workspace,
            format!("pdxi_{label}"),
            label.to_string(),
            executable.clone(),
        );
        let run = manager
            .start("sleep".to_string(), test_super_args())
            .unwrap();
        managers.push(manager);
        runs.push(run.run_id);
    }
    std::thread::sleep(Duration::from_millis(100));
    managers[1].cancel(&runs[1]);
    let cancelled = wait_for_terminal(&managers[1], &runs[1]);
    assert_eq!(cancelled.summary.state, ExposeRunState::Cancelled);
    assert!(!managers[0].status(&runs[0]).unwrap().state.terminal());
    assert!(!managers[2].status(&runs[2]).unwrap().state.terminal());
    for manager in &managers {
        manager.shutdown();
    }
}
