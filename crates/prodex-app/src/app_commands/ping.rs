use anyhow::{Result, bail};
use codex_config::codex_config_value;
use prodex_cli::{PingCommands, PingOpenaiArgs};
use prodex_core::AppPaths;
use prodex_quota::usage_has_spark_limit;
use prodex_state::{AppState, ProfileProvider};
use std::ffi::OsString;
use std::path::Path;
use terminal_ui::print_stdout_line;

use super::{
    codex_child_plan, collect_run_profile_reports, prepare_codex_launch_args,
    ready_profile_candidates, run_child_plan, runtime_launch_openai_spark_context_codex_args,
};
use crate::app_state::{AppStateIoExt, repair_missing_active_profile_and_save};

const OPENAI_CODEX_SPARK_MODEL: &str = "gpt-5.3-codex-spark";

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
    let mut ready_profiles = ready_profile_candidates(
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
    .map(|candidate| (candidate.name, candidate.usage))
    .collect::<Vec<_>>();
    ready_profiles.sort_by(|left, right| left.0.cmp(&right.0));

    if ready_profiles.is_empty() {
        print_stdout_line("No ready OpenAI profiles.")?;
        return Ok(());
    }

    let mut failures = Vec::new();
    for (profile_name, usage) in ready_profiles {
        let Some(profile) = state.profiles.get(&profile_name) else {
            continue;
        };
        let models = ping_openai_models_for_usage(&profile.codex_home, &usage)?;
        for model in models {
            match model {
                Some(model) => print_stdout_line(&format!("Pinging {profile_name} ({model})...")),
                None => print_stdout_line(&format!("Pinging {profile_name}...")),
            }?;
            let plan = ping_openai_child_plan(profile.codex_home.clone(), model)?;
            let status = run_child_plan(&plan, None)?;
            if !status.success() {
                failures.push((
                    match model {
                        Some(model) => format!("{profile_name} ({model})"),
                        None => profile_name.clone(),
                    },
                    status.code().unwrap_or(1),
                ));
            }
        }
    }

    if failures.is_empty() {
        print_stdout_line("Ping complete.")?;
        return Ok(());
    }

    let summary = failures
        .into_iter()
        .map(|(name, code)| format!("{name} exited {code}"))
        .collect::<Vec<_>>()
        .join(", ");
    bail!("ping failed for {summary}")
}

fn ping_openai_models_for_usage(
    codex_home: &Path,
    usage: &crate::UsageResponse,
) -> Result<Vec<Option<&'static str>>> {
    let mut models = vec![None];
    if usage_has_spark_limit(usage) && !ping_openai_default_model_is_spark(codex_home)? {
        models.push(Some(OPENAI_CODEX_SPARK_MODEL));
    }
    Ok(models)
}

fn ping_openai_default_model_is_spark(codex_home: &Path) -> Result<bool> {
    Ok(
        codex_config_value(codex_home, "model")?.is_some_and(|model| {
            matches!(
                model.trim().to_ascii_lowercase().as_str(),
                "gpt-5.3-codex-spark" | "gpt-5.3-spark"
            )
        }),
    )
}

fn ping_openai_child_plan(
    codex_home: std::path::PathBuf,
    model: Option<&str>,
) -> Result<crate::ChildProcessPlan> {
    let mut args = Vec::new();
    if let Some(model) = model {
        args.push(OsString::from("--model"));
        args.push(OsString::from(model));
    }
    args.extend([OsString::from("exec"), OsString::from("ping")]);
    let (args, _) = prepare_codex_launch_args(&args, true);
    let args = runtime_launch_openai_spark_context_codex_args(&codex_home, &args)?;
    Ok(codex_child_plan(codex_home, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_openai_child_plan_bypasses_approvals_and_sandbox() {
        let plan = ping_openai_child_plan("/tmp/codex-home".into(), None).unwrap();
        assert_eq!(
            plan.args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("exec"),
                OsString::from("ping")
            ]
        );
    }

    #[test]
    fn ping_openai_child_plan_can_target_spark() {
        let plan = ping_openai_child_plan("/tmp/codex-home".into(), Some(OPENAI_CODEX_SPARK_MODEL))
            .unwrap();
        assert_eq!(
            plan.args,
            vec![
                OsString::from("--dangerously-bypass-approvals-and-sandbox"),
                OsString::from("--model"),
                OsString::from(OPENAI_CODEX_SPARK_MODEL),
                OsString::from("exec"),
                OsString::from("-c"),
                OsString::from("model_context_window=128000"),
                OsString::from("-c"),
                OsString::from("model_auto_compact_token_limit=115200"),
                OsString::from("ping")
            ]
        );
    }
}
