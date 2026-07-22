use super::{
    PreparedRuntimeLaunch, RuntimeLaunchPlan, RuntimeLaunchRequest, RuntimeProxyEndpoint,
    cleanup_runtime_launch_plan, exit_with_status, run_child_plan, run_child_plan_with_monitor,
};
use crate::print_launch_status;
use anyhow::Result;
use std::process::ExitStatus;

pub(crate) trait RuntimeLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_>;
    fn harness_mode(&self) -> Option<prodex_provider_core::HarnessMode> {
        None
    }
    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan>;
    fn child_exit_requested(&mut self) -> bool {
        false
    }
    fn monitors_child_exit(&self) -> bool {
        false
    }
    fn session_affinity_release(&self) -> Option<&str> {
        None
    }
    fn relaunch_after_child_exit(&mut self, _status: &ExitStatus) -> Result<bool> {
        Ok(false)
    }
    fn after_child_exit(&mut self, _status: &ExitStatus) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct RuntimeLaunchExecution {
    plan: RuntimeLaunchPlan,
    runtime_proxy: Option<RuntimeProxyEndpoint>,
}

struct RuntimeLaunchCompleted {
    status: ExitStatus,
    plan: RuntimeLaunchPlan,
}

pub(crate) fn execute_runtime_launch<S>(mut strategy: S) -> Result<()>
where
    S: RuntimeLaunchStrategy,
{
    loop {
        let execution = build_runtime_launch_execution(&strategy)?;
        let completed = if strategy.monitors_child_exit() {
            run_runtime_launch_execution(execution, Some(&mut || strategy.child_exit_requested()))?
        } else {
            run_runtime_launch_execution(execution, None)?
        };
        let relaunch = strategy.relaunch_after_child_exit(&completed.status)?;
        let after_result = strategy.after_child_exit(&completed.status);
        cleanup_runtime_launch_plan(&completed.plan);
        after_result?;
        if relaunch {
            continue;
        }
        return exit_with_status(completed.status);
    }
}

fn build_runtime_launch_execution<S>(strategy: &S) -> Result<RuntimeLaunchExecution>
where
    S: RuntimeLaunchStrategy,
{
    let request = strategy.runtime_request();
    emit_runtime_launch_progress(&request);
    let resolved_harness =
        prodex_provider_core::resolve_harness_mode(strategy.harness_mode(), None);
    let prepared = super::prepare_runtime_launch_with_harness(request, resolved_harness)?;
    if let (Some(runtime_proxy), Some(session_id)) = (
        prepared.runtime_proxy.as_ref(),
        strategy.session_affinity_release(),
    ) {
        runtime_proxy.release_session_affinity(session_id)?;
    }
    let plan = strategy.build_plan(&prepared, prepared.runtime_proxy.as_ref())?;
    Ok(RuntimeLaunchExecution {
        plan,
        runtime_proxy: prepared.runtime_proxy,
    })
}

fn emit_runtime_launch_progress(request: &RuntimeLaunchRequest<'_>) {
    print_launch_status("preparing runtime launch...");
    if request.presidio_redaction_enabled {
        print_launch_status("Presidio redaction requested; preparing local redaction proxy.");
    }
    if request.smart_context_enabled {
        print_launch_status("Smart Context runtime proxy requested.");
    }
    if runtime_launch_uses_kiro_connect_proxy(request) {
        print_launch_status("authenticated local Kiro transport tunnel requested.");
    }
    let proxied_external_provider = request.external_provider.is_some_and(|provider| {
        !provider.eq_ignore_ascii_case("kiro") && !provider.eq_ignore_ascii_case("antigravity")
    });
    if request.model_provider_override.is_some() || proxied_external_provider {
        print_launch_status("local provider bridge requested.");
    }
}

pub(crate) fn runtime_launch_uses_kiro_connect_proxy(request: &RuntimeLaunchRequest<'_>) -> bool {
    request
        .external_provider
        .is_some_and(|provider| provider.eq_ignore_ascii_case("kiro"))
        && request.model_provider_override.is_none()
}

fn run_runtime_launch_execution(
    execution: RuntimeLaunchExecution,
    child_exit_requested: Option<&mut dyn FnMut() -> bool>,
) -> Result<RuntimeLaunchCompleted> {
    let RuntimeLaunchExecution {
        plan,
        runtime_proxy,
    } = execution;
    print_launch_status("starting child process...");
    let status = match child_exit_requested {
        Some(monitor) => run_child_plan_with_monitor(&plan.child, runtime_proxy.as_ref(), monitor),
        None => run_child_plan(&plan.child, runtime_proxy.as_ref()),
    };
    drop(runtime_proxy);
    match status {
        Ok(status) => Ok(RuntimeLaunchCompleted { status, plan }),
        Err(err) => {
            cleanup_runtime_launch_plan(&plan);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChildProcessPlan;
    use std::ffi::OsString;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn run_runtime_launch_execution_defers_cleanup_until_after_child_exit_work() {
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-execution-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cleanup_path = root.join("overlay-home");
        fs::create_dir_all(&cleanup_path).unwrap();

        let plan = RuntimeLaunchPlan::new(
            ChildProcessPlan::new(OsString::from("/bin/sh"), root.join("codex-home"))
                .with_args(vec![OsString::from("-c"), OsString::from("exit 0")]),
        )
        .with_cleanup_path(cleanup_path.clone());

        let completed = run_runtime_launch_execution(
            RuntimeLaunchExecution {
                plan,
                runtime_proxy: None,
            },
            None,
        )
        .expect("child should run");

        assert!(completed.status.success());
        assert!(
            cleanup_path.exists(),
            "cleanup path must remain available for after_child_exit maintenance"
        );

        cleanup_runtime_launch_plan(&completed.plan);
        assert!(
            !cleanup_path.exists(),
            "cleanup path should be removable after after_child_exit work"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn run_runtime_launch_execution_stops_a_live_child_when_requested() {
        let root = std::env::temp_dir().join(format!(
            "prodex-runtime-execution-monitor-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let plan = RuntimeLaunchPlan::new(
            ChildProcessPlan::new(OsString::from("/bin/sh"), root.join("codex-home")).with_args(
                vec![
                    OsString::from("-c"),
                    OsString::from("trap 'exit 42' TERM; while :; do sleep 1; done"),
                ],
            ),
        );
        let mut polls = 0;

        let completed = run_runtime_launch_execution(
            RuntimeLaunchExecution {
                plan,
                runtime_proxy: None,
            },
            Some(&mut || {
                polls += 1;
                polls >= 3
            }),
        )
        .expect("monitored child should stop");

        assert!(!completed.status.success());
        assert!(polls >= 3);
        cleanup_runtime_launch_plan(&completed.plan);
        let _ = fs::remove_dir_all(root);
    }
}
