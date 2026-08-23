use prodex_session_store::SessionReport;
use terminal_ui::SessionReportDisplay;

pub fn session_report_display_rows(reports: &[SessionReport]) -> Vec<SessionReportDisplay<'_>> {
    reports
        .iter()
        .map(|report| SessionReportDisplay {
            id: &report.id,
            updated_at: report.updated_at.as_deref(),
            thread_name: report.thread_name.as_deref(),
            cwd: report.cwd.as_deref(),
            profile: report.profile.as_deref(),
            path: &report.path,
        })
        .collect()
}

pub fn render_session_reports_text(reports: &[SessionReport]) -> String {
    terminal_ui::render_session_reports(&session_report_display_rows(reports))
}

pub fn render_session_reports_output(
    reports: &[SessionReport],
    json: bool,
    empty_message: &str,
) -> Result<String, serde_json::Error> {
    if json {
        return serde_json::to_string_pretty(reports);
    }

    if reports.is_empty() {
        return Ok(terminal_ui::render_panel(
            "Sessions",
            &[("Status".to_string(), empty_message.to_string())],
        ));
    }

    Ok(render_session_reports_text(reports))
}

#[cfg(test)]
#[path = "../../tests/src/reports/session.rs"]
mod tests;
