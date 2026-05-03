use super::*;
pub(crate) use prodex_app_reports::SessionReport;
use prodex_app_reports::render_session_reports_output;

pub(crate) fn handle_session(command: SessionCommands) -> Result<()> {
    match command {
        SessionCommands::List(args) => {
            let reports = load_session_reports(None, args.limit)?;
            print_session_reports(&reports, args.json, "No sessions found")
        }
        SessionCommands::Current(args) => {
            let cwd =
                args.cwd.map(absolutize).transpose()?.unwrap_or(
                    env::current_dir().context("failed to determine current directory")?,
                );
            let reports = load_session_reports(Some(&cwd), args.limit)?;
            print_session_reports(
                &reports,
                args.json,
                &format!("No sessions found for {}", cwd.display()),
            )
        }
    }
}

fn load_session_reports(
    current_dir: Option<&Path>,
    limit: Option<usize>,
) -> Result<Vec<SessionReport>> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let mut reports = prodex_session_store::collect_session_reports(
        &paths.shared_codex_root,
        current_dir,
        &state,
    )?;
    if let Some(limit) = limit {
        reports.truncate(limit);
    }
    Ok(reports)
}

fn print_session_reports(reports: &[SessionReport], json: bool, empty_message: &str) -> Result<()> {
    let output = render_session_reports_output(reports, json, empty_message)
        .context("failed to render session JSON")?;
    if json || reports.is_empty() {
        print_stdout_line(&output);
    } else {
        print_stdout_text(&output);
    }
    Ok(())
}
