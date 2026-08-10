use super::{
    RuntimeLaunchDryRunChild, configure_child_process_group, join_thread_with_timeout,
    profile_openai_compatible_dry_run_child, runtime_launch_dry_run_tui_text,
    runtime_launch_dry_run_value_color, runtime_launch_harness_dry_run_line,
    terminate_child_process_group_best_effort, terminate_child_process_tree,
};
use ratatui::style::Color;
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn runtime_launch_dry_run_tui_text_keeps_report_content() {
    let text = runtime_launch_dry_run_tui_text(
        "Command: codex\nRuntime proxy: enabled\nPresidio redaction: disabled\n",
    );
    let rendered = text
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Command: codex"));
    assert!(rendered.contains("Runtime proxy: enabled"));
    assert!(rendered.contains("Presidio redaction: disabled"));
}

#[test]
fn harness_dry_run_line_reports_immutable_resolution() {
    let resolved = prodex_provider_core::resolve_harness_mode(
        Some(prodex_provider_core::HarnessMode::Minimal),
        None,
    );

    let line = runtime_launch_harness_dry_run_line(&resolved);

    assert!(line.contains("requested=minimal"), "{line}");
    assert!(line.contains("resolved=minimal"), "{line}");
    assert!(line.contains("source=cli"), "{line}");
    assert!(line.contains("reason=explicit CLI selection"), "{line}");
}

#[test]
fn runtime_launch_dry_run_value_color_highlights_status() {
    assert_eq!(
        runtime_launch_dry_run_value_color("Runtime proxy", "enabled"),
        Color::Green
    );
    assert_eq!(
        runtime_launch_dry_run_value_color("Presidio redaction", "disabled"),
        Color::Red
    );
    assert_eq!(
        runtime_launch_dry_run_value_color("Command", "codex"),
        Color::Cyan
    );
}

#[test]
fn dry_run_rejects_profile_url_secret_before_building_child_plan() {
    let root = std::env::temp_dir().join(format!(
        "prodex-dry-run-url-boundary-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(".prodex-profile.toml"),
        "openai_compatible_base_url = 'https://user:dry-run-plan-secret-sentinel@example.test/v1'\n",
    )
    .unwrap();

    let error = profile_openai_compatible_dry_run_child(
        &root,
        RuntimeLaunchDryRunChild::Codex {
            codex_args: Vec::new(),
        },
    )
    .unwrap_err()
    .to_string();

    assert!(
        error.contains("no credentials, query, or fragment"),
        "{error}"
    );
    assert!(!error.contains("secret-sentinel"), "{error}");
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn generic_child_process_uses_private_process_group() {
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30"]);
    configure_child_process_group(&mut command, true);
    let mut child = command.spawn().expect("test child should spawn");
    let pid = child.id() as libc::pid_t;
    let process_group = unsafe { libc::getpgid(pid) };

    let _ = terminate_child_process_tree(&mut child, true);
    let _ = child.wait();

    assert_eq!(process_group, pid);
}

#[cfg(unix)]
#[test]
fn private_process_group_cleanup_survives_reaped_leader() {
    let pid_file = std::env::temp_dir().join(format!(
        "prodex-child-group-{}-{}.pid",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "sleep 30 & echo $! > \"$1\"; exit 0",
        "sh",
        pid_file.to_str().unwrap(),
    ]);
    configure_child_process_group(&mut command, true);
    let mut child = command.spawn().expect("test child should spawn");
    let shell_pid = child.id() as libc::pid_t;
    let status = child.wait().expect("test child should exit");
    let descendant_pid = fs::read_to_string(&pid_file)
        .expect("descendant pid should be written")
        .trim()
        .parse::<libc::pid_t>()
        .expect("descendant pid should be numeric");
    let descendant_group = unsafe { libc::getpgid(descendant_pid) };

    let group_signal_succeeded = terminate_child_process_group_best_effort(&child, true);
    let _ = fs::remove_file(pid_file);

    assert!(status.success());
    assert_eq!(descendant_group, shell_pid);
    assert!(
        group_signal_succeeded,
        "private process-group signal failed"
    );
}

#[cfg(unix)]
#[test]
fn interactive_child_keeps_inherited_process_group() {
    let parent_process_group = unsafe { libc::getpgrp() };
    let mut command = Command::new("sh");
    command.args(["-c", "sleep 30"]);
    configure_child_process_group(&mut command, false);
    let mut child = command.spawn().expect("test child should spawn");
    let process_group = unsafe { libc::getpgid(child.id() as libc::pid_t) };

    let _ = terminate_child_process_tree(&mut child, false);
    let _ = child.wait();

    assert_eq!(process_group, parent_process_group);
}

#[test]
fn sub_agent_child_inherits_the_launcher_process_group() {
    let _marker = crate::test_support::TestEnvVarGuard::set(crate::SUB_AGENT_RECURSION_MARKER, "1");

    assert!(!super::child_owns_private_process_group());
}

#[test]
fn thread_join_timeout_is_bounded() {
    let started = Instant::now();
    let worker = std::thread::spawn(|| std::thread::sleep(Duration::from_millis(100)));

    let error =
        join_thread_with_timeout(worker, Duration::from_millis(10), "test worker").unwrap_err();

    assert!(error.to_string().contains("did not stop"));
    assert!(started.elapsed() < Duration::from_secs(1));
}
