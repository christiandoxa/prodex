use std::io::IsTerminal;

use crate::{ExportProfileArgs, ProfileExportPayload, print_stderr_line, print_stderr_prompt};
use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::path::Path;
use std::{env, io};
use zeroize::Zeroizing;

use super::progress::print_profile_import_progress;
use terminal_ui::{
    tui_border_style, tui_connected_footer_block, tui_connected_header_block, tui_detail_style,
    tui_hint_style, tui_primary_style, tui_secondary_style, tui_success_style, tui_title_style,
};

const PROFILE_EXPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_EXPORT_PASSWORD";
const PROFILE_IMPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_IMPORT_PASSWORD";

pub(super) fn resolve_export_password_mode(args: &ExportProfileArgs) -> Result<bool> {
    if args.password_protect {
        return Ok(true);
    }
    if args.no_password {
        return Ok(false);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "non-interactive profile export requires --password-protect with {} set, or --no-password to write an unencrypted bundle",
            PROFILE_EXPORT_PASSWORD_ENV
        );
    }
    prompt_export_password_mode_tui().or_else(|_| {
        prompt_yes_no(
            "Password-protect export file containing profile tokens? [Y/n]: ",
            true,
        )
    })
}

pub(super) fn resolve_export_password() -> Result<Zeroizing<String>> {
    if let Ok(password) = env::var(PROFILE_EXPORT_PASSWORD_ENV)
        && !password.trim().is_empty()
    {
        return Ok(Zeroizing::new(password));
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "password protection requested but no interactive terminal is available; set {}",
            PROFILE_EXPORT_PASSWORD_ENV
        );
    }

    let password = Zeroizing::new(prompt_profile_export_password_tui(
        "Profile Export",
        "Export password",
        "Enter a password for the encrypted profile bundle.",
    )?);
    if password.is_empty() {
        bail!("export password cannot be empty");
    }
    let confirmation = Zeroizing::new(prompt_profile_export_password_tui(
        "Profile Export",
        "Confirm export password",
        "Enter the same password again.",
    )?);
    if password != confirmation {
        bail!("export passwords did not match");
    }
    Ok(password)
}

pub(super) fn resolve_import_password() -> Result<String> {
    if let Ok(password) = env::var(PROFILE_IMPORT_PASSWORD_ENV)
        && !password.trim().is_empty()
    {
        return Ok(password);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "profile export bundle is password-protected; set {} or rerun in a terminal",
            PROFILE_IMPORT_PASSWORD_ENV
        );
    }

    let password = prompt_profile_export_password_tui(
        "Profile Import",
        "Export password",
        "Enter the password for this encrypted profile bundle.",
    )?;
    if password.is_empty() {
        bail!("import password cannot be empty");
    }
    Ok(password)
}

fn prompt_profile_export_password_tui(title: &str, label: &str, detail: &str) -> Result<String> {
    let mut tui = ExportPromptTui::stderr("profile export prompt TUI")?;
    let mut input = Zeroizing::new(String::new());
    loop {
        tui.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled(title.to_string(), tui_title_style()),
                Span::raw("  "),
                Span::styled(label.to_string(), tui_secondary_style()),
            ]))
            .block(tui_connected_header_block(tui_border_style()));
            frame.render_widget(header, chunks[0]);

            let body = Paragraph::new(vec![
                Line::from(Span::styled(
                    label.to_string(),
                    tui_primary_style().add_modifier(Modifier::BOLD),
                )),
                Line::raw(""),
                Line::from(Span::styled(detail.to_string(), tui_secondary_style())),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("> ", tui_hint_style()),
                    Span::styled("*".repeat(input.chars().count()), tui_primary_style()),
                    Span::styled("_", tui_hint_style()),
                ]),
            ])
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(body, chunks[1]);

            let footer = Paragraph::new(Line::from(vec![
                Span::styled("enter", tui_hint_style()),
                Span::raw(" accept  "),
                Span::styled("backspace", tui_hint_style()),
                Span::raw(" delete  "),
                Span::styled("esc", tui_hint_style()),
                Span::raw(" cancel"),
            ]))
            .block(tui_connected_footer_block(tui_border_style()));
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Enter => return Ok(std::mem::take(&mut *input)),
                KeyCode::Esc => bail!("profile export password input cancelled"),
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    bail!("profile export password input cancelled");
                }
                KeyCode::Char(ch) => input.push(ch),
                _ => {}
            }
        }
    }
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let mut input = String::new();
    loop {
        print_stderr_prompt(prompt)?;
        input.clear();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read prompt response")?;
        match input.trim().to_ascii_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                print_stderr_line("Please answer yes or no.")?;
            }
        }
    }
}

type ExportPromptTui = terminal_ui::AlternateScreenTerminal<io::Stderr>;

fn prompt_export_password_mode_tui() -> Result<bool> {
    let mut tui = ExportPromptTui::stderr("profile export prompt TUI")?;
    loop {
        tui.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(4),
                    Constraint::Length(3),
                ])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled("Profile Export", tui_title_style()),
                Span::raw("  "),
                Span::styled("bundle protection", tui_detail_style()),
            ]))
            .block(
                tui_connected_header_block(tui_border_style()),
            );
            frame.render_widget(header, chunks[0]);

            let body = Paragraph::new(vec![
                Line::from(Span::styled(
                    "Password-protect export file containing profile tokens?",
                    tui_primary_style().add_modifier(Modifier::BOLD),
                )),
                Line::raw(""),
                Line::from(Span::styled(
                    "Protected bundles require a password to import. Unprotected bundles are plain JSON and may contain reusable credentials.",
                    tui_secondary_style(),
                )),
            ])
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(body, chunks[1]);

            let footer = Paragraph::new(Line::from(vec![
                Span::styled("y", tui_success_style()),
                Span::raw(" protect  "),
                Span::styled("enter", tui_success_style()),
                Span::raw(" protect  "),
                Span::styled("n", tui_hint_style()),
                Span::raw(" unprotected  "),
                Span::styled("esc", tui_hint_style()),
                Span::raw(" unprotected"),
            ]))
            .block(
                tui_connected_footer_block(tui_border_style()),
            );
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => return Ok(true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => return Ok(false),
                KeyCode::Char('c') | KeyCode::Char('z')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    return Ok(false);
                }
                _ => {}
            }
        }
    }
}

pub(super) fn read_profile_export_payload(path: &Path) -> Result<(ProfileExportPayload, bool)> {
    print_profile_import_status("Reading profile export bundle...");
    let (envelope, encrypted) = prodex_profile_export::read_profile_export_envelope(path)?;
    let payload = prodex_profile_export::decode_profile_export_envelope(envelope, || {
        let password = resolve_import_password()?;
        print_profile_import_status("Decrypting encrypted profile export...");
        Ok(password)
    })?;
    Ok((payload, encrypted))
}

fn print_profile_import_status(message: &str) {
    let _ = print_profile_import_progress(message);
}
