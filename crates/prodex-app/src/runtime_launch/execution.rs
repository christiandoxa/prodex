use super::{
    PreparedRuntimeLaunch, RuntimeLaunchPlan, RuntimeLaunchRequest, RuntimeProxyEndpoint,
    cleanup_runtime_launch_plan, exit_with_status, run_child_plan,
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
    fn relaunch_after_child_exit(&mut self, _status: &ExitStatus) -> Result<bool> {
        Ok(false)
    }
    fn after_child_exit(&mut self, _status: &ExitStatus) -> Result<()> {
        Ok(())
    }
}

pub(crate) trait RuntimeLaunchPlanFactory {
    fn build_runtime_launch_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan>;
}

impl<T> RuntimeLaunchPlanFactory for T
where
    T: RuntimeLaunchStrategy,
{
    fn build_runtime_launch_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        self.build_plan(prepared, runtime_proxy)
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

struct RuntimeLaunchExecutionBuilder<'a, F> {
    prepared: PreparedRuntimeLaunch,
    plan_factory: &'a F,
}

impl<'a, F> RuntimeLaunchExecutionBuilder<'a, F>
where
    F: RuntimeLaunchPlanFactory,
{
    fn new(prepared: PreparedRuntimeLaunch, plan_factory: &'a F) -> Self {
        Self {
            prepared,
            plan_factory,
        }
    }

    fn build(self) -> Result<RuntimeLaunchExecution> {
        let RuntimeLaunchExecutionBuilder {
            prepared,
            plan_factory,
        } = self;
        let plan = {
            let runtime_proxy = prepared.runtime_proxy.as_ref();
            plan_factory.build_runtime_launch_plan(&prepared, runtime_proxy)?
        };
        let runtime_proxy = prepared.runtime_proxy;
        Ok(RuntimeLaunchExecution {
            plan,
            runtime_proxy,
        })
    }
}

pub(crate) fn execute_runtime_launch<S>(mut strategy: S) -> Result<()>
where
    S: RuntimeLaunchStrategy,
{
    loop {
        let execution = build_runtime_launch_execution(&strategy)?;
        let completed = run_runtime_launch_execution(execution)?;
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
    RuntimeLaunchExecutionBuilder::new(prepared, strategy).build()
}

fn emit_runtime_launch_progress(request: &RuntimeLaunchRequest<'_>) {
    print_launch_status("preparing runtime and Prodex overlay...");
    if request.presidio_redaction_enabled {
        print_launch_status("Presidio redaction requested; preparing local redaction proxy.");
    }
    if request.smart_context_enabled {
        print_launch_status("Smart Context runtime proxy requested.");
    }
    if request.model_provider_override.is_some() || request.external_provider.is_some() {
        print_launch_status("local provider bridge requested.");
    }
}

fn run_runtime_launch_execution(
    execution: RuntimeLaunchExecution,
) -> Result<RuntimeLaunchCompleted> {
    let RuntimeLaunchExecution {
        plan,
        runtime_proxy,
    } = execution;
    print_launch_status("starting child process...");
    let status = run_child_plan(&plan.child, runtime_proxy.as_ref());
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

        let completed = run_runtime_launch_execution(RuntimeLaunchExecution {
            plan,
            runtime_proxy: None,
        })
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
}
