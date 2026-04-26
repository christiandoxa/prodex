use super::*;

mod audit;
mod broker;
mod child_process;
mod cleanup;
mod doctor;
mod info;
mod info_handler;
mod quota;
mod runtime_launch;
mod selection;
mod shared;

pub(crate) use self::audit::*;
pub(crate) use self::broker::*;
pub(crate) use self::child_process::*;
pub(crate) use self::cleanup::*;
pub(crate) use self::doctor::*;
pub(crate) use self::info::*;
pub(crate) use self::info_handler::*;
pub(crate) use self::quota::*;
pub(crate) use self::selection::*;
pub(crate) use self::shared::*;

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    runtime_launch::handle_run(args)
}

pub(super) fn handle_super(args: SuperArgs) -> Result<()> {
    handle_caveman(args.into_caveman_args())
}

pub(super) fn prepare_runtime_launch(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    runtime_launch::prepare_runtime_launch(request)
}

pub(super) fn prepare_runtime_launch_dry_run(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    runtime_launch::prepare_runtime_launch_dry_run(request)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    runtime_launch::resolve_runtime_launch_profile_name(state, requested)
}
