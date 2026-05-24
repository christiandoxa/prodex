use super::*;
use std::io::IsTerminal;

mod audit;
mod broker;
mod capability;
mod child_process;
mod cleanup;
mod context;
mod doctor;
mod info;
mod info_handler;
mod presidio;
mod quota;
mod runtime_launch;
mod selection;
mod session;
mod shared;

pub(crate) use self::audit::*;
pub(crate) use self::broker::*;
pub(crate) use self::capability::*;
pub(crate) use self::child_process::*;
pub(crate) use self::cleanup::*;
pub(crate) use self::context::*;
pub(crate) use self::doctor::*;
pub(crate) use self::info::*;
pub(crate) use self::info_handler::*;
pub(crate) use self::presidio::*;
pub(crate) use self::quota::*;
pub(crate) use self::selection::*;
pub(crate) use self::session::*;
pub(crate) use self::shared::*;

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    runtime_launch::handle_run(args)
}

pub(super) fn handle_super(args: SuperArgs) -> Result<()> {
    let use_presidio = prompt_super_presidio_opt_in()?;
    handle_caveman(args.into_caveman_args_with_presidio(use_presidio))
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

fn prompt_super_presidio_opt_in() -> Result<bool> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Ok(false);
    }

    eprint!("Use Presidio for data safety? [y/N] ");
    io::stderr().flush().context("failed to flush prompt")?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("failed to read Presidio prompt answer")?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
