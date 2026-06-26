use anyhow::{Context, Result, bail};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::{
    AppPaths, AppState, AppStateIoExt, QuotaArgs, QuotaAuthFilter, QuotaProviderFilter,
    collect_quota_reports, collect_quota_reports_with_filters, fetch_profile_quota,
    fetch_profile_quota_json, print_stdout_line, print_stdout_text, quota_watch_enabled,
    render_profile_quota_snapshot, render_quota_reports, repair_missing_active_profile_and_save,
    resolve_profile_name, watch_all_quotas, watch_quota,
};

pub(crate) fn handle_quota(args: QuotaArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load_and_repair(&paths)?;
    repair_missing_active_profile_and_save(&paths, &mut state)?;
    let auth_filter = args
        .auth
        .as_deref()
        .map(QuotaAuthFilter::parse)
        .transpose()?
        .unwrap_or(QuotaAuthFilter::All);
    let provider_filter = args
        .provider
        .as_deref()
        .map(QuotaProviderFilter::parse)
        .transpose()?
        .unwrap_or(QuotaProviderFilter::All);
    let provider_filter_locked =
        args.provider.is_some() && provider_filter != QuotaProviderFilter::All;

    if args.all {
        if state.profiles.is_empty()
            && !matches!(
                provider_filter,
                QuotaProviderFilter::DeepSeek | QuotaProviderFilter::Local
            )
        {
            bail!("no profiles configured");
        }
        if quota_watch_enabled(&args) {
            return watch_all_quotas(
                &paths,
                args.base_url.as_deref(),
                args.detail,
                auth_filter,
                provider_filter,
                provider_filter_locked,
            );
        }
        let reports = if matches!(auth_filter, QuotaAuthFilter::All)
            && matches!(provider_filter, QuotaProviderFilter::All)
        {
            collect_quota_reports(&state, args.base_url.as_deref())
        } else {
            collect_quota_reports_with_filters(
                &state,
                args.base_url.as_deref(),
                &auth_filter,
                provider_filter,
            )
        };
        print_quota_human("Quota", &render_quota_reports(&reports, args.detail))?;
        return Ok(());
    }

    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let profile = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let codex_home = profile.codex_home.clone();

    if args.raw {
        let usage =
            fetch_profile_quota_json(&profile.provider, &codex_home, args.base_url.as_deref())?;
        let json = serde_json::to_string_pretty(&usage).context("failed to render usage JSON")?;
        print_stdout_line(&json);
        return Ok(());
    }

    if quota_watch_enabled(&args) {
        return watch_quota(
            &profile_name,
            &profile.provider,
            &codex_home,
            args.base_url.as_deref(),
        );
    }

    let quota = fetch_profile_quota(&profile.provider, &codex_home, args.base_url.as_deref())?;
    print_quota_human(
        "Quota",
        &render_profile_quota_snapshot(&profile_name, &quota),
    )?;
    Ok(())
}

fn print_quota_human(title: &str, output: &str) -> Result<()> {
    let height = output.lines().count().saturating_add(4).clamp(6, 28) as u16;
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        print_stdout_text(output);
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "Prodex Quota",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(title.to_string(), Style::default().fg(Color::DarkGray)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(output.to_string())
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}
