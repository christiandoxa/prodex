use std::io::{self, IsTerminal};

use anyhow::{Context, Result, bail};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use prodex_quota::{UsageResponse, UsageWindow, format_precise_reset_time};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::{
    AppPaths, AppState, AppStateIoExt, ProfileProvider, RateLimitResetCreditConsumeFlow,
    RateLimitResetCreditConsumeOutcome, RedeemArgs, fetch_usage_with_proxy_policy,
    print_stderr_line, print_stderr_prompt, print_stdout_line,
    repair_missing_active_profile_and_save,
};
use terminal_ui::{
    print_panel, tui_border_style, tui_detail_style, tui_hint_style, tui_primary_style,
    tui_secondary_style, tui_success_style, tui_title_style,
};

const MANUAL_REDEEM_NEAR_RESET_SECONDS: i64 = 60 * 60;

pub(crate) fn handle_redeem(args: RedeemArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load_and_repair(&paths)?;
    repair_missing_active_profile_and_save(&paths, &mut state)?;
    let profile = state
        .profiles
        .get(&args.profile)
        .with_context(|| format!("profile '{}' is missing", args.profile))?;
    if !matches!(profile.provider, ProfileProvider::Openai) {
        bail!(
            "profile '{}' is not an OpenAI/Codex profile and cannot redeem reset credits",
            args.profile
        );
    }

    let usage = fetch_usage_with_proxy_policy(
        &profile.codex_home,
        args.base_url.as_deref(),
        args.no_proxy,
    )?;
    confirm_manual_redeem_if_reset_near(&args.profile, &usage, args.yes)?;

    let redeem_request_id = manual_redeem_request_id();
    let response = RateLimitResetCreditConsumeFlow::new_with_proxy_policy(
        &profile.codex_home,
        args.base_url.as_deref(),
        args.no_proxy,
    )?
    .execute(&redeem_request_id)?;

    print_redeem_result(
        &args.profile,
        manual_redeem_outcome_label(response.outcome),
        &redeem_request_id,
    );
    Ok(())
}

fn confirm_manual_redeem_if_reset_near(
    profile_name: &str,
    usage: &UsageResponse,
    assume_yes: bool,
) -> Result<()> {
    let Some(near_reset) = nearest_manual_redeem_reset(
        usage,
        chrono::Local::now().timestamp(),
        MANUAL_REDEEM_NEAR_RESET_SECONDS,
    ) else {
        return Ok(());
    };

    if assume_yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "profile '{}' has a {} reset near at {}; rerun in a terminal to confirm or pass --yes",
            profile_name,
            near_reset.label,
            format_precise_reset_time(Some(near_reset.reset_at))
        );
    }

    if !prompt_manual_redeem_confirmation(
        profile_name,
        near_reset.label,
        &format_precise_reset_time(Some(near_reset.reset_at)),
    )? {
        bail!("redeem cancelled");
    }
    Ok(())
}

fn nearest_manual_redeem_reset(
    usage: &UsageResponse,
    now: i64,
    near_reset_seconds: i64,
) -> Option<ManualRedeemNearReset> {
    let rate_limit = usage.rate_limit.as_ref()?;
    [
        ("5h", rate_limit.primary_window.as_ref()),
        ("weekly", rate_limit.secondary_window.as_ref()),
    ]
    .into_iter()
    .filter_map(|(label, window)| {
        manual_redeem_window_near_reset(label, window?, now, near_reset_seconds)
    })
    .min_by_key(|reset| reset.reset_at)
}

fn manual_redeem_window_near_reset(
    label: &'static str,
    window: &UsageWindow,
    now: i64,
    near_reset_seconds: i64,
) -> Option<ManualRedeemNearReset> {
    let reset_at = window.reset_at?;
    let seconds_until_reset = reset_at.saturating_sub(now);
    if seconds_until_reset <= near_reset_seconds {
        return Some(ManualRedeemNearReset { label, reset_at });
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ManualRedeemNearReset {
    label: &'static str,
    reset_at: i64,
}

fn prompt_manual_redeem_confirmation(
    profile_name: &str,
    reset_label: &str,
    reset_time: &str,
) -> Result<bool> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        return prompt_manual_redeem_confirmation_tui(profile_name, reset_label, reset_time);
    }

    print_stderr_line(&format!(
        "Profile '{}' has a {} reset near at {}.",
        profile_name, reset_label, reset_time
    ));
    let mut input = String::new();
    loop {
        print_stderr_prompt("Redeem one reset credit anyway? [y/N]: ")?;
        input.clear();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read redeem confirmation")?;
        match parse_manual_redeem_confirmation(&input) {
            Some(confirmed) => return Ok(confirmed),
            None => print_stderr_line("Please answer yes or no."),
        }
    }
}

fn print_redeem_result(profile_name: &str, outcome: &str, request_id: &str) {
    if !io::stdout().is_terminal() {
        print_stdout_line(&format!(
            "profile={profile_name} outcome={outcome} request_id={request_id}"
        ));
        return;
    }

    let fields = vec![
        ("Profile".to_string(), profile_name.to_string()),
        ("Outcome".to_string(), outcome.to_string()),
        ("Request".to_string(), request_id.to_string()),
    ];
    if print_redeem_result_tui(&fields).is_err() {
        print_panel("Redeem", &fields);
    }
}

fn print_redeem_result_tui(fields: &[(String, String)]) -> Result<()> {
    let Some(mut terminal) = crate::try_inline_stdout_terminal(7) else {
        anyhow::bail!("stdout is not an inline-capable terminal");
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Prodex Redeem", tui_title_style()),
            Span::raw("  "),
            Span::styled("reset credit", tui_detail_style()),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(tui_border_style()),
        );
        frame.render_widget(header, chunks[0]);

        let lines = fields
            .iter()
            .map(|(label, value)| {
                Line::from(vec![
                    Span::styled(format!("{label:>9} "), tui_detail_style()),
                    Span::styled(
                        value.clone(),
                        if label == "Outcome" {
                            tui_success_style()
                        } else {
                            tui_primary_style()
                        },
                    ),
                ])
            })
            .collect::<Vec<_>>();
        let body = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

struct RedeemPromptTui {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
}

impl RedeemPromptTui {
    fn new() -> Result<Self> {
        enable_raw_mode().context("failed to enable redeem prompt TUI raw mode")?;
        let mut stderr = io::stderr();
        if let Err(err) = crossterm::execute!(stderr, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(err).context("failed to enter redeem prompt TUI alternate screen");
        }
        let backend = CrosstermBackend::new(stderr);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let mut stderr = io::stderr();
                let _ = crossterm::execute!(stderr, Show, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err).context("failed to initialize redeem prompt TUI terminal");
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for RedeemPromptTui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn prompt_manual_redeem_confirmation_tui(
    profile_name: &str,
    reset_label: &str,
    reset_time: &str,
) -> Result<bool> {
    let mut tui = RedeemPromptTui::new()?;
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
                Span::styled("Prodex Redeem", tui_title_style()),
                Span::raw("  "),
                Span::styled("manual reset credit", tui_secondary_style()),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(tui_border_style()),
            );
            frame.render_widget(header, chunks[0]);

            let body = Paragraph::new(vec![
                Line::from(Span::styled(
                    "Quota reset is near.",
                    tui_primary_style().add_modifier(Modifier::BOLD),
                )),
                Line::raw(""),
                Line::from(format!(
                    "Profile '{profile_name}' has a {reset_label} reset near at {reset_time}."
                )),
                Line::from("Redeem one reset credit anyway?"),
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
                Span::raw(" redeem  "),
                Span::styled("n", tui_hint_style()),
                Span::raw(" cancel  "),
                Span::styled("enter", tui_hint_style()),
                Span::raw(" cancel  "),
                Span::styled("esc", tui_hint_style()),
                Span::raw(" cancel"),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(tui_border_style()),
            );
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter | KeyCode::Esc => {
                    return Ok(false);
                }
                _ => {}
            }
        }
    }
}

fn parse_manual_redeem_confirmation(input: &str) -> Option<bool> {
    match input.trim().to_ascii_lowercase().as_str() {
        "" | "n" | "no" => Some(false),
        "y" | "yes" => Some(true),
        _ => None,
    }
}

fn manual_redeem_request_id() -> String {
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("prodex-manual-redeem-{}-{now_nanos}", std::process::id())
}

fn manual_redeem_outcome_label(outcome: RateLimitResetCreditConsumeOutcome) -> &'static str {
    match outcome {
        RateLimitResetCreditConsumeOutcome::Reset => "reset",
        RateLimitResetCreditConsumeOutcome::NothingToReset => "nothing-to-reset",
        RateLimitResetCreditConsumeOutcome::NoCredit => "no-credit",
        RateLimitResetCreditConsumeOutcome::AlreadyRedeemed => "already-redeemed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_redeem_request_id_is_prodex_scoped() {
        assert!(manual_redeem_request_id().starts_with("prodex-manual-redeem-"));
    }

    #[test]
    fn manual_redeem_outcome_labels_are_human_readable() {
        assert_eq!(
            manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::Reset),
            "reset"
        );
        assert_eq!(
            manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::NothingToReset),
            "nothing-to-reset"
        );
        assert_eq!(
            manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::NoCredit),
            "no-credit"
        );
        assert_eq!(
            manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::AlreadyRedeemed),
            "already-redeemed"
        );
    }

    fn usage_with_resets(five_hour_reset_at: i64, weekly_reset_at: i64) -> UsageResponse {
        serde_json::from_value(serde_json::json!({
            "rate_limit": {
                "primary_window": {
                    "used_percent": 20,
                    "reset_at": five_hour_reset_at,
                    "limit_window_seconds": 18000
                },
                "secondary_window": {
                    "used_percent": 30,
                    "reset_at": weekly_reset_at,
                    "limit_window_seconds": 604800
                }
            }
        }))
        .expect("usage response should parse")
    }

    #[test]
    fn manual_redeem_allows_remaining_quota_when_reset_is_not_near() {
        let usage = usage_with_resets(1_000 + MANUAL_REDEEM_NEAR_RESET_SECONDS + 1, 604_800);
        assert_eq!(
            nearest_manual_redeem_reset(&usage, 1_000, MANUAL_REDEEM_NEAR_RESET_SECONDS),
            None
        );
    }

    #[test]
    fn manual_redeem_detects_nearest_near_reset() {
        let usage = usage_with_resets(1_060, 1_030);
        assert_eq!(
            nearest_manual_redeem_reset(&usage, 1_000, MANUAL_REDEEM_NEAR_RESET_SECONDS),
            Some(ManualRedeemNearReset {
                label: "weekly",
                reset_at: 1_030
            })
        );
    }

    #[test]
    fn manual_redeem_confirmation_defaults_to_no() {
        assert_eq!(parse_manual_redeem_confirmation(""), Some(false));
        assert_eq!(parse_manual_redeem_confirmation("no"), Some(false));
        assert_eq!(parse_manual_redeem_confirmation("yes"), Some(true));
        assert_eq!(parse_manual_redeem_confirmation("wat"), None);
    }
}
