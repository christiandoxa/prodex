use crate::CLI_TABLE_GAP;
use crate::panel::section_header_with_width;
use crate::terminal::current_cli_width;
use crate::text::{pad_cell, text_width};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReportDisplay<'a> {
    pub id: &'a str,
    pub updated_at: Option<&'a str>,
    pub thread_name: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub profile: Option<&'a str>,
    pub path: &'a str,
}

pub fn render_session_reports(reports: &[SessionReportDisplay<'_>]) -> String {
    render_session_reports_with_width(reports, current_cli_width())
}

pub fn render_session_reports_with_width(
    reports: &[SessionReportDisplay<'_>],
    total_width: usize,
) -> String {
    let widths = session_report_column_widths(total_width);
    let mut lines = vec![section_header_with_width("Sessions", total_width)];
    lines.push(format_session_report_row(
        "ID", "UPDATED", "THREAD", "CWD", "PATH", widths,
    ));
    lines.push("-".repeat(text_width(lines.last().map(String::as_str).unwrap_or(""))));

    for report in reports {
        lines.push(format_session_report_row(
            report.id,
            report.updated_at.unwrap_or("-"),
            report.thread_name.unwrap_or("-"),
            report.cwd.unwrap_or("-"),
            report.path,
            widths,
        ));
        if text_width(report.id) > widths.id {
            lines.push(format!("  id: {}", report.id));
        }
        if let Some(profile) = report.profile {
            lines.push(format!("  profile: {profile}"));
        }
    }

    lines.join("\n")
}

#[derive(Clone, Copy)]
struct SessionReportColumnWidths {
    id: usize,
    updated: usize,
    thread: usize,
    cwd: usize,
    path: usize,
}

fn session_report_column_widths(total_width: usize) -> SessionReportColumnWidths {
    let gap_width = text_width(CLI_TABLE_GAP) * 4;
    let available = total_width.saturating_sub(gap_width).max(5);
    let id = (available / 5).clamp(1, 26);
    let updated = (available / 5).clamp(1, 22);
    let thread = (available / 5).clamp(1, 24);
    let remaining = available.saturating_sub(id + updated + thread);
    let cwd = remaining / 2;
    let path = remaining.saturating_sub(cwd);
    SessionReportColumnWidths {
        id,
        updated,
        thread,
        cwd,
        path,
    }
}

fn format_session_report_row(
    id: &str,
    updated: &str,
    thread_name: &str,
    cwd: &str,
    path: &str,
    widths: SessionReportColumnWidths,
) -> String {
    format!(
        "{}{gap}{}{gap}{}{gap}{}{gap}{}",
        pad_cell(id, widths.id),
        pad_cell(updated, widths.updated),
        pad_cell(thread_name, widths.thread),
        pad_cell(cwd, widths.cwd),
        pad_cell(path, widths.path),
        gap = CLI_TABLE_GAP,
    )
}
