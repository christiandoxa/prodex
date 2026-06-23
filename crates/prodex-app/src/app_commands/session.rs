use anyhow::{Context, Result};
use std::env;
use std::ffi::OsString;
use std::path::Path;

use crate::{
    AppPaths, AppState, AppStateIoExt, RunArgs, SessionCommands, SessionResumeArgs, absolutize,
    handle_run, print_stdout_line, print_stdout_text,
};
pub(crate) use prodex_app_reports::SessionReport;
use prodex_app_reports::render_session_reports_output;

pub(crate) fn handle_session(command: SessionCommands) -> Result<()> {
    match command {
        SessionCommands::List(args) => {
            let output_mode = session_output_mode(args.json, args.id_only, args.resume_command)?;
            let reports = load_session_reports(
                None,
                args.limit,
                args.profile.as_deref(),
                args.query.as_deref(),
                args.include_subagents,
            )?;
            print_session_reports(&reports, output_mode, "No sessions found")
        }
        SessionCommands::Current(args) => {
            let output_mode = session_output_mode(args.json, args.id_only, args.resume_command)?;
            let cwd =
                args.cwd.map(absolutize).transpose()?.unwrap_or(
                    env::current_dir().context("failed to determine current directory")?,
                );
            let reports = load_session_reports(
                Some(&cwd),
                args.limit,
                args.profile.as_deref(),
                args.query.as_deref(),
                args.include_subagents,
            )?;
            print_session_reports(
                &reports,
                output_mode,
                &format!("No sessions found for {}", cwd.display()),
            )
        }
        SessionCommands::Resume(args) => handle_session_resume(args),
    }
}

fn load_session_reports(
    current_dir: Option<&Path>,
    limit: Option<usize>,
    profile: Option<&str>,
    query: Option<&str>,
    include_subagents: bool,
) -> Result<Vec<SessionReport>> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let mut reports = prodex_session_store::collect_session_reports_with_filter(
        &paths.shared_codex_root,
        prodex_session_store::SessionReportFilter {
            current_dir,
            profile,
            query,
            include_subagents,
        },
        &state,
    )?;
    if let Some(limit) = limit {
        reports.truncate(limit);
    }
    Ok(reports)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionOutputMode {
    Text,
    Json,
    IdOnly,
    ResumeCommand,
}

fn session_output_mode(
    json: bool,
    id_only: bool,
    resume_command: bool,
) -> Result<SessionOutputMode> {
    let selected = [json, id_only, resume_command]
        .into_iter()
        .filter(|selected| *selected)
        .count();
    if selected > 1 {
        return Err(anyhow::anyhow!(
            "--json, --id-only, and --resume-command cannot be combined"
        ));
    }

    if json {
        Ok(SessionOutputMode::Json)
    } else if id_only {
        Ok(SessionOutputMode::IdOnly)
    } else if resume_command {
        Ok(SessionOutputMode::ResumeCommand)
    } else {
        Ok(SessionOutputMode::Text)
    }
}

fn print_session_reports(
    reports: &[SessionReport],
    output_mode: SessionOutputMode,
    empty_message: &str,
) -> Result<()> {
    match output_mode {
        SessionOutputMode::IdOnly => {
            print_session_lines(reports.iter().map(|report| report.id.clone()));
            Ok(())
        }
        SessionOutputMode::ResumeCommand => {
            print_session_lines(
                reports
                    .iter()
                    .map(|report| format!("prodex run {}", report.id)),
            );
            Ok(())
        }
        SessionOutputMode::Json | SessionOutputMode::Text => {
            let json = output_mode == SessionOutputMode::Json;
            let output = render_session_reports_output(reports, json, empty_message)
                .context("failed to render session JSON")?;
            if json || reports.is_empty() {
                print_stdout_line(&output);
            } else {
                print_stdout_text(&output);
            }
            Ok(())
        }
    }
}

fn print_session_lines(lines: impl IntoIterator<Item = String>) {
    let mut output = String::new();
    for line in lines {
        output.push_str(&line);
        output.push('\n');
    }
    if !output.is_empty() {
        print_stdout_text(&output);
    }
}

fn handle_session_resume(args: SessionResumeArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let _ = prodex_session_store::repair_resume_session_metadata_prefix(
        &paths.shared_codex_root,
        &args.id,
    )?;
    if let Some(path) =
        prodex_session_store::find_unrepairable_resume_session(&paths.shared_codex_root, &args.id)?
    {
        anyhow::bail!(
            "session '{}' cannot be resumed because {} does not contain session metadata; the file is too incomplete to repair",
            args.id,
            path.display()
        );
    }
    let reports =
        prodex_session_store::collect_session_reports(&paths.shared_codex_root, None, &state)?;
    let report = prodex_session_store::resolve_session_report_by_id(&reports, &args.id)
        .map_err(anyhow::Error::new)?;

    handle_run(RunArgs {
        profile: None,
        auto_rotate: false,
        no_auto_rotate: false,
        auto_redeem: false,
        skip_quota_check: false,
        full_access: false,
        base_url: None,
        no_proxy: false,
        dry_run: false,
        codex_args: vec![OsString::from("resume"), OsString::from(report.id.clone())],
    })
}
