use crate::{ChildProcessPlan, RuntimeLaunchPlan, RuntimeProxyEndpoint};
use anyhow::Result;
#[cfg(unix)]
use anyhow::{Context, bail};
use std::process::ExitStatus;

pub(crate) fn run_runtime_launch_plan(
    plan: &RuntimeLaunchPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    monitor: Option<&mut dyn FnMut() -> Result<bool>>,
) -> Result<ExitStatus> {
    #[cfg(unix)]
    {
        run_runtime_launch_plan_unix(plan, runtime_proxy, monitor)
    }
    #[cfg(not(unix))]
    {
        match monitor {
            Some(monitor) => {
                super::run_child_plan_with_monitor(&plan.child, runtime_proxy, monitor)
            }
            None => super::run_child_plan(&plan.child, runtime_proxy),
        }
    }
}

#[cfg(unix)]
fn run_runtime_launch_plan_unix(
    plan: &RuntimeLaunchPlan,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    monitor: Option<&mut dyn FnMut() -> Result<bool>>,
) -> Result<ExitStatus> {
    let Some(companion_plan) = plan.companion.as_ref() else {
        return match monitor {
            Some(monitor) => {
                super::run_child_plan_with_monitor(&plan.child, runtime_proxy, monitor)
            }
            None => super::run_child_plan(&plan.child, runtime_proxy),
        };
    };
    if let Some(socket) = plan.companion_unix_socket.as_ref() {
        let _ = std::fs::remove_file(socket);
    }
    let private_process_group = super::child_owns_private_process_group();
    let mut companion = spawn_companion(companion_plan, private_process_group)?;
    let companion_lease = match runtime_proxy {
        Some(proxy) => match proxy.create_child_lease(companion.id()) {
            Ok(lease) => Some(lease),
            Err(error) => {
                let _ = super::terminate_child_process_tree(&mut companion, private_process_group);
                let _ = super::wait_for_child(&mut companion, private_process_group);
                return Err(error);
            }
        },
        None => None,
    };
    let ready = plan
        .companion_unix_socket
        .as_ref()
        .is_some_and(|path| wait_for_unix_socket(path, &mut companion));
    if !ready {
        let _ = super::terminate_child_process_tree(&mut companion, private_process_group);
        let _ = super::wait_for_child(&mut companion, private_process_group);
        drop(companion_lease);
        bail!("Codex app-server companion did not become ready")
    }
    let status = match monitor {
        Some(monitor) => super::run_child_plan_with_monitor(&plan.child, runtime_proxy, monitor),
        None => super::run_child_plan(&plan.child, runtime_proxy),
    };
    let _ = super::terminate_child_process_tree(&mut companion, private_process_group);
    let _ = super::wait_for_child(&mut companion, private_process_group);
    if let Some(socket) = plan.companion_unix_socket.as_ref() {
        let _ = std::fs::remove_file(socket);
    }
    drop(companion_lease);
    status
}

#[cfg(unix)]
fn spawn_companion(
    plan: &ChildProcessPlan,
    private_process_group: bool,
) -> Result<std::process::Child> {
    super::cleanup_codex_arg0_temp_dirs_best_effort(&plan.codex_home);
    let mut command = std::process::Command::new(&plan.binary);
    command
        .args(&plan.args)
        .env("CODEX_HOME", &plan.codex_home)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    for key in &plan.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &plan.extra_env {
        command.env(key, value);
    }
    super::configure_child_process_group(&mut command, private_process_group);
    command.spawn().with_context(|| {
        format!(
            "failed to execute companion {}",
            plan.binary.to_string_lossy()
        )
    })
}

#[cfg(unix)]
fn wait_for_unix_socket(path: &std::path::Path, child: &mut std::process::Child) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return true;
        }
        if child.try_wait().ok().flatten().is_some() {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}
