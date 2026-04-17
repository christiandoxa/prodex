use super::*;

pub(crate) trait RuntimeLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_>;
    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan>;
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

struct RuntimeLaunchExecutionFactory<'a, F> {
    request: RuntimeLaunchRequest<'a>,
    plan_factory: &'a F,
}

impl<'a, F> RuntimeLaunchExecutionFactory<'a, F>
where
    F: RuntimeLaunchPlanFactory,
{
    fn new(request: RuntimeLaunchRequest<'a>, plan_factory: &'a F) -> Self {
        Self {
            request,
            plan_factory,
        }
    }

    fn build(self) -> Result<RuntimeLaunchExecution> {
        let prepared = prepare_runtime_launch(self.request)?;
        RuntimeLaunchExecutionBuilder::new(prepared, self.plan_factory).build()
    }
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

pub(crate) fn execute_runtime_launch<S>(strategy: S) -> Result<()>
where
    S: RuntimeLaunchStrategy,
{
    let execution = build_runtime_launch_execution(&strategy)?;
    exit_with_status(run_runtime_launch_execution(execution)?)
}

fn build_runtime_launch_execution<S>(strategy: &S) -> Result<RuntimeLaunchExecution>
where
    S: RuntimeLaunchStrategy,
{
    RuntimeLaunchExecutionFactory::new(strategy.runtime_request(), strategy).build()
}

fn run_runtime_launch_execution(execution: RuntimeLaunchExecution) -> Result<ExitStatus> {
    let RuntimeLaunchExecution {
        plan,
        runtime_proxy,
    } = execution;
    let status = run_child_plan(&plan.child, runtime_proxy.as_ref());
    drop(runtime_proxy);
    cleanup_runtime_launch_plan(&plan);
    status
}
