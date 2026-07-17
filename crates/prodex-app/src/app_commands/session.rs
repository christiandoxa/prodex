use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Wrap;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal};
use std::path::Path;
use terminal_ui::{
    tui_border_style, tui_connected_footer_block, tui_connected_header_block, tui_detail_style,
    tui_hint_style, tui_primary_style, tui_secondary_style, tui_title_style,
};

use crate::{
    AppPaths, AppState, AppStateIoExt, RunArgs, SessionCommands, SessionResumeArgs, absolutize,
    handle_run, print_stdout_line, print_stdout_text,
};
use prodex_app_reports::render_session_reports_output;
use prodex_cli::CodexRuntimeFeatureArgs;
pub(crate) use prodex_session_store::SessionReport;

pub(crate) fn handle_session(command: SessionCommands) -> Result<()> {
    match command {
        SessionCommands::List(args) => {
            let output_mode = session_output_mode(args.json, args.id_only, args.resume_command)?;
            let reports = load_session_reports(
                None,
                args.limit,
                args.profile.as_deref(),
                args.query.as_deref(),
                !args.parent_only,
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
                !args.parent_only,
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
            print_session_lines(reports.iter().map(|report| report.id.clone()))?;
            Ok(())
        }
        SessionOutputMode::ResumeCommand => {
            print_session_lines(
                reports
                    .iter()
                    .map(|report| format!("prodex run {}", report.id)),
            )?;
            Ok(())
        }
        SessionOutputMode::Json | SessionOutputMode::Text => {
            let json = output_mode == SessionOutputMode::Json;
            let output = render_session_reports_output(reports, json, empty_message)
                .context("failed to render session JSON")?;
            if json {
                print_stdout_line(&output)?;
            } else if io::stdout().is_terminal() {
                render_session_reports_tui(reports, empty_message)?;
            } else {
                print_stdout_text(&output)?;
            }
            Ok(())
        }
    }
}

fn render_session_reports_tui(reports: &[SessionReport], empty_message: &str) -> Result<()> {
    // Try scrollable TUI first (like profile list)
    let total_lines = session_tui_item_count(reports);
    let term_h = usize::from(terminal::size().map(|(_, h)| h).unwrap_or(24));
    let needs_scroll = total_lines.saturating_add(6) > term_h
        && env::var_os("CODEX_CI").is_none()
        && !env::var("CI")
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);

    if needs_scroll {
        return render_session_reports_tui_scrollable(reports, empty_message);
    }

    // Inline (short list) — identical to before
    let height = session_report_tui_height(reports);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        let output = render_session_reports_output(reports, false, empty_message)
            .context("failed to render session fallback output")?;
        print_stdout_text(&output)?;
        return Ok(());
    };
    terminal
        .draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(2),
                ])
                .split(frame.area());

            let header = Paragraph::new(Line::from(vec![
                Span::styled("Prodex Sessions", tui_title_style()),
                Span::raw("  "),
                Span::styled(
                    format!("{} session(s)", reports.len()),
                    tui_secondary_style(),
                ),
            ]))
            .block(tui_connected_header_block(tui_border_style()));
            frame.render_widget(header, chunks[0]);

            if reports.is_empty() {
                let empty =
                    Paragraph::new(Line::styled(empty_message.to_string(), tui_detail_style()))
                        .block(
                            Block::default()
                                .borders(Borders::LEFT | Borders::RIGHT)
                                .border_style(tui_border_style()),
                        );
                frame.render_widget(empty, chunks[1]);
            } else {
                let items = reports
                    .iter()
                    .map(session_report_tui_item)
                    .collect::<Vec<_>>();
                let list = List::new(items).block(
                    Block::default()
                        .borders(Borders::LEFT | Borders::RIGHT)
                        .border_style(tui_border_style()),
                );
                frame.render_widget(list, chunks[1]);
            }

            let footer = Paragraph::new(Line::styled(
                "use `prodex run <session-id>` to resume",
                tui_hint_style(),
            ))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(tui_border_style()),
            );
            frame.render_widget(footer, chunks[2]);
        })
        .context("failed to draw session report TUI")?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn session_tui_item_count(reports: &[SessionReport]) -> usize {
    reports.len().saturating_mul(4)
}

fn render_session_reports_tui_scrollable(
    reports: &[SessionReport],
    empty_message: &str,
) -> Result<()> {
    let mut terminal = terminal_ui::AlternateScreenTerminal::stdout("session list TUI")?;

    let mut scroll_offset = 0usize;
    (|| -> Result<()> {
        loop {
            let lines: Vec<Line<'_>> = session_scroll_lines(reports);
            let total = lines.len();
            let size = terminal.size()?;
            let visible = usize::from(size.height).saturating_sub(6).max(1);
            let max_scroll = total.saturating_sub(visible);
            scroll_offset = scroll_offset.min(max_scroll);

            terminal.draw(|frame| {
                let chunks = Layout::default().direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)])
                    .split(frame.area());

                let header = Paragraph::new(Line::from(vec![
                    Span::styled("Prodex Sessions", tui_title_style()),
                    Span::raw("  "),
                    Span::styled(format!("{} session(s)", reports.len()), tui_secondary_style()),
                ])).block(tui_connected_header_block(tui_border_style()));
                frame.render_widget(header, chunks[0]);

                if reports.is_empty() {
                    let empty = Paragraph::new(Line::styled(empty_message.to_string(), tui_secondary_style()))
                        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT).border_style(tui_border_style()));
                    frame.render_widget(empty, chunks[1]);
                } else {
                    let slice: Vec<Line<'_>> = lines.iter().skip(scroll_offset).take(visible).cloned().collect();
                    let body = Paragraph::new(Text::from(slice))
                        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT).border_style(tui_border_style()))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(body, chunks[1]);
                }

                let footer_text = if max_scroll == 0 {
                    format!("{} session(s) — q / Esc / Enter to exit", reports.len())
                } else {
                    format!(
                        "lines {}-{} of {} — j/k/Up/Down/PgUp/PgDn/Home/End to scroll, q/Esc/Enter to exit",
                        scroll_offset.saturating_add(1),
                        (scroll_offset + visible).min(total),
                        total,
                    )
                };
                let footer = Paragraph::new(Line::styled(footer_text, tui_hint_style().add_modifier(Modifier::BOLD)))
                    .block(tui_connected_footer_block(tui_border_style()));
                frame.render_widget(footer, chunks[2]);
            }).context("failed to draw session scroll TUI")?;

            if let Event::Key(key) = crossterm::event::read().context("failed to read input")?
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => break,
                    KeyCode::Char('c') | KeyCode::Char('z')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        break;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        scroll_offset = scroll_offset.saturating_add(1).min(max_scroll)
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        scroll_offset = scroll_offset.saturating_sub(1)
                    }
                    KeyCode::PageDown => {
                        scroll_offset = scroll_offset.saturating_add(visible).min(max_scroll)
                    }
                    KeyCode::PageUp => scroll_offset = scroll_offset.saturating_sub(visible),
                    KeyCode::Home => scroll_offset = 0,
                    KeyCode::End => scroll_offset = max_scroll,
                    _ => {}
                }
            }
        }
        Ok(())
    })()
}

fn session_scroll_lines(reports: &[SessionReport]) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    for report in reports {
        let title = report
            .thread_name
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("Untitled session");
        let updated = report.updated_at.as_deref().unwrap_or("-");
        let profile = report.profile.as_deref().unwrap_or("-");
        let provider = report.model_provider.as_deref().unwrap_or("-");
        lines.push(Line::from(vec![
            Span::styled(
                report.id.as_str(),
                tui_hint_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(title, tui_primary_style()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("updated ", tui_secondary_style()),
            Span::raw(updated),
            Span::styled("  profile ", tui_secondary_style()),
            Span::raw(profile),
            Span::styled("  provider ", tui_secondary_style()),
            Span::raw(provider),
        ]));
        if let Some(cwd) = &report.cwd {
            lines.push(Line::from(vec![
                Span::styled("cwd ", tui_secondary_style()),
                Span::raw(cwd.as_str()),
            ]));
        }
        lines.push(Line::raw(""));
    }
    lines
}

fn session_report_tui_height(reports: &[SessionReport]) -> u16 {
    let rows = reports.len().saturating_mul(4).saturating_add(5).max(8);
    let terminal_height = terminal::size()
        .map(|(_, height)| usize::from(height))
        .unwrap_or(24);
    rows.min(terminal_height).max(1) as u16
}

fn session_report_tui_item(report: &SessionReport) -> ListItem<'_> {
    let title = report
        .thread_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Untitled session");
    let updated = report.updated_at.as_deref().unwrap_or("-");
    let profile = report.profile.as_deref().unwrap_or("-");
    let cwd = report.cwd.as_deref().unwrap_or("-");
    let provider = report.model_provider.as_deref().unwrap_or("-");
    ListItem::new(Text::from(vec![
        Line::from(vec![
            Span::styled(
                report.id.as_str(),
                tui_hint_style().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(title.to_string(), tui_primary_style()),
        ]),
        Line::from(vec![
            Span::styled("updated ", tui_secondary_style()),
            Span::raw(updated.to_string()),
            Span::styled(" profile ", tui_secondary_style()),
            Span::raw(profile.to_string()),
            Span::styled(" provider ", tui_secondary_style()),
            Span::raw(provider.to_string()),
        ]),
        Line::from(vec![
            Span::styled("cwd ", tui_secondary_style()),
            Span::raw(cwd.to_string()),
        ]),
        Line::raw(""),
    ]))
    .style(Style::default())
}

fn print_session_lines(lines: impl IntoIterator<Item = String>) -> io::Result<()> {
    let mut output = String::new();
    for line in lines {
        output.push_str(&line);
        output.push('\n');
    }
    if !output.is_empty() {
        print_stdout_text(&output)?;
    }
    Ok(())
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
    let report = prodex_session_store::resolve_session_report_by_id_in_store(
        &paths.shared_codex_root,
        &state,
        &args.id,
    )
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
        codex_features: CodexRuntimeFeatureArgs::default(),
        codex_args: vec![OsString::from("resume"), OsString::from(report.id.clone())],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session_report(id: &str) -> SessionReport {
        let mut report = SessionReport::from_path(Path::new(&format!("/tmp/{id}.jsonl")), 0);
        prodex_session_store::apply_session_json_line(
            &mut report,
            r#"{"timestamp":"2026-06-26T10:00:00Z","type":"session_meta","payload":{"thread_name":"Build UI","cwd":"/tmp/prodex"}}"#,
        );
        report.set_profile(Some("main".to_string()));
        report.set_model_provider(Some("openai".to_string()));
        report
    }

    #[test]
    fn session_report_tui_height_scales_with_reports() {
        assert!(session_report_tui_height(&[] as &[SessionReport]) >= 1);
        let reports = vec![test_session_report("a"), test_session_report("b")];
        assert!(usize::from(session_report_tui_height(&reports)) >= 8);
        assert_eq!(session_tui_item_count(&reports), 8);
    }

    #[test]
    fn session_report_tui_item_contains_key_fields() {
        let report = test_session_report("session-1");
        let item = session_report_tui_item(&report);
        let text = format!("{item:?}");
        assert!(text.contains("session-1"));
        assert!(text.contains("Build UI"));
        assert!(text.contains("main"));
        assert!(text.contains("openai"));
    }
}
