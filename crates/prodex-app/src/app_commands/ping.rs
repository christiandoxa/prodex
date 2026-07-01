use anyhow::{Result, bail};
use prodex_cli::{PingCommands, PingOpenaiArgs};
use prodex_core::AppPaths;
use prodex_state::{AppState, ProfileProvider};
use std::ffi::OsString;
use terminal_ui::print_stdout_line;

use super::{
    codex_child_plan, collect_run_profile_reports, prepare_codex_launch_args,
    ready_profile_candidates, run_child_plan,
};
use crate::app_state::{AppStateIoExt, repair_missing_active_profile_and_save};

pub(crate) fn handle_ping(command: PingCommands) -> Result<()> {
    match command {
        PingCommands::Openai(args) => handle_ping_openai(args),
    }
}

fn handle_ping_openai(args: PingOpenaiArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load_and_repair(&paths)?;
    repair_missing_active_profile_and_save(&paths, &mut state)?;

    let profile_names = state
        .profiles
        .iter()
        .filter(|(_, profile)| matches!(profile.provider, ProfileProvider::Openai))
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let mut ready_profile_names = ready_profile_candidates(
        &collect_run_profile_reports(
            &state,
            profile_names,
            args.base_url.as_deref(),
            args.no_proxy,
        ),
        false,
        None,
        &state,
        None,
    )
    .into_iter()
    .map(|candidate| candidate.name)
    .collect::<Vec<_>>();
    ready_profile_names.sort();

    if ready_profile_names.is_empty() {
        print_stdout_line("No ready OpenAI profiles.");
        return Ok(());
    }

    let mut failures = Vec::new();
    for profile_name in ready_profile_names {
        let Some(profile) = state.profiles.get(&profile_name) else {
            continue;
        };
        print_stdout_line(&format!("Pinging {profile_name}..."));
        let plan = ping_openai_child_plan(profile.codex_home.clone());
        let status = run_child_plan(&plan, None)?;
        if !status.success() {
            failures.push((profile_name, status.code().unwrap_or(1)));
        }
    }

    if failures.is_empty() {
        print_stdout_line("Ping complete.");
        return Ok(());
    }

    let summary = failures
        .into_iter()
        .map(|(name, code)| format!("{name} exited {code}"))
        .collect::<Vec<_>>()
        .join(", ");
    bail!("ping failed for {summary}")
}

fn ping_openai_child_plan(codex_home: std::path::PathBuf) -> crate::ChildProcessPlan {
    let (args, _) =
        prepare_codex_launch_args(&[OsString::from("exec"), OsString::from("ping")], true);
    codex_child_plan(codex_home, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_openai_child_plan_bypasses_approvals_and_sandbox() {
        let plan = ping_openai_child_plan("/tmp/codex-home".into());
        assert_eq!(
            plan.args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("exec"),
                OsString::from("ping")
            ]
        );
    }
}
