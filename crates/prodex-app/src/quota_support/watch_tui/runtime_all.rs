use super::runtime_profile::{
    QuotaWatchTui, print_quota_watch_plain_snapshot, quota_watch_quit_key,
};
use super::*;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::io::IsTerminal;

pub(crate) fn watch_all_quotas(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
    auth_filter: QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> Result<()> {
    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        match watch_all_quotas_tui(
            paths,
            base_url,
            detail,
            auth_filter.clone(),
            provider_filter,
            provider_filter_locked,
        ) {
            Ok(()) => return Ok(()),
            Err(err) if std::env::var_os("PRODEX_TUI_STRICT").is_none() => {
                eprintln!("{}", quota_watch_tui_fallback_message(&err));
            }
            Err(err) => return Err(err),
        }
    }

    watch_all_quotas_plain(
        paths,
        base_url,
        detail,
        auth_filter,
        provider_filter,
        provider_filter_locked,
    )
}

fn watch_all_quotas_plain(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
    auth_filter: QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> Result<()> {
    let mut scroll_offset = 0_usize;
    let sort = QuotaReportSort::Current;
    let collection_provider_filter = if provider_filter_locked {
        provider_filter
    } else {
        QuotaProviderFilter::All
    };
    let mut snapshot = AllQuotaWatchSnapshot::Loading {
        updated: quota_watch_updated_at(),
    };
    let mut refresh = AllQuotaWatchRefresh::new();
    let _ = start_all_quota_watch_refresh(
        &mut refresh,
        paths,
        base_url,
        &auth_filter,
        collection_provider_filter,
    );
    let mut redraw_needed = true;
    let mut next_refresh_at = None;
    let mut auth_backoff_profiles = std::collections::BTreeSet::new();
    let mut next_auth_backoff_poll_at = Instant::now();

    loop {
        if Instant::now() >= next_auth_backoff_poll_at {
            let next_auth_backoff_profiles = quota_runtime_auth_backoff_profiles();
            if next_auth_backoff_profiles != auth_backoff_profiles {
                auth_backoff_profiles = next_auth_backoff_profiles;
                redraw_needed = true;
            }
            next_auth_backoff_poll_at =
                Instant::now() + Duration::from_secs(ALL_QUOTA_WATCH_AUTH_BACKOFF_POLL_SECONDS);
        }

        if let Some(next_snapshot) = refresh.take_latest() {
            snapshot = merge_all_quota_watch_snapshot(&snapshot, next_snapshot);
            redraw_needed = true;
            next_refresh_at = Some(all_quota_watch_next_refresh_at(
                paths,
                &snapshot,
                detail,
                &auth_filter,
                collection_provider_filter,
            ));
        }

        if redraw_needed {
            let render_snapshot =
                quota_watch_snapshot_with_auth_backoff(&snapshot, &auth_backoff_profiles);
            scroll_offset = scroll_offset.min(quota_watch_max_scroll_offset(
                &render_snapshot,
                detail,
                provider_filter,
                sort,
            ));
            let output = render_all_quota_watch_snapshot(
                &render_snapshot,
                detail,
                scroll_offset,
                sort,
                provider_filter,
                provider_filter_locked,
            );
            print_quota_watch_plain_snapshot(&output)?;
            redraw_needed = false;
        }

        if next_refresh_at.is_some_and(|refresh_at| Instant::now() >= refresh_at)
            && start_all_quota_watch_refresh(
                &mut refresh,
                paths,
                base_url,
                &auth_filter,
                collection_provider_filter,
            )
        {
            next_refresh_at = None;
            continue;
        }

        thread::sleep(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS));
    }
}

fn watch_all_quotas_tui(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
    auth_filter: QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> Result<()> {
    let mut tui = QuotaWatchTui::stdout("quota TUI")?;
    let mut scroll_offset = 0_usize;
    let mut sort = QuotaReportSort::Current;
    let mut provider_filter = provider_filter;
    let collection_provider_filter = if provider_filter_locked {
        provider_filter
    } else {
        QuotaProviderFilter::All
    };
    let mut snapshot = AllQuotaWatchSnapshot::Loading {
        updated: quota_watch_updated_at(),
    };
    let mut refresh = AllQuotaWatchRefresh::new();
    let _ = start_all_quota_watch_refresh(
        &mut refresh,
        paths,
        base_url,
        &auth_filter,
        collection_provider_filter,
    );
    let mut redraw_needed = true;
    let mut next_refresh_at = None;
    let mut auth_backoff_profiles = std::collections::BTreeSet::new();
    let mut next_auth_backoff_poll_at = Instant::now();

    loop {
        if Instant::now() >= next_auth_backoff_poll_at {
            let next_auth_backoff_profiles = quota_runtime_auth_backoff_profiles();
            if next_auth_backoff_profiles != auth_backoff_profiles {
                auth_backoff_profiles = next_auth_backoff_profiles;
                redraw_needed = true;
            }
            next_auth_backoff_poll_at =
                Instant::now() + Duration::from_secs(ALL_QUOTA_WATCH_AUTH_BACKOFF_POLL_SECONDS);
        }

        if let Some(next_snapshot) = refresh.take_latest() {
            snapshot = merge_all_quota_watch_snapshot(&snapshot, next_snapshot);
            redraw_needed = true;
            next_refresh_at = Some(all_quota_watch_next_refresh_at(
                paths,
                &snapshot,
                detail,
                &auth_filter,
                collection_provider_filter,
            ));
        }

        if redraw_needed {
            let size = tui
                .terminal
                .size()
                .context("failed to read quota TUI terminal size")?;
            let render_snapshot =
                quota_watch_snapshot_with_auth_backoff(&snapshot, &auth_backoff_profiles);
            let max_lines = quota_watch_tui_table_lines(
                size.height,
                quota_watch_snapshot_overview_field_count(&render_snapshot, provider_filter),
            );
            scroll_offset = scroll_offset.min(quota_watch_tui_max_scroll_offset_for_snapshot(
                &render_snapshot,
                detail,
                provider_filter,
                sort,
                max_lines,
            ));
            let frame = build_all_quota_watch_tui_frame(
                &render_snapshot,
                AllQuotaWatchLayout {
                    detail,
                    scroll_offset,
                    sort,
                    provider_filter,
                    provider_filter_locked,
                    total_width: usize::from(size.width).saturating_sub(4),
                    max_lines,
                },
            );
            tui.terminal
                .draw(|area| render_all_quota_watch_tui(area, &frame))
                .context("failed to draw quota TUI")?;
            redraw_needed = false;
        }

        if next_refresh_at.is_some_and(|refresh_at| Instant::now() >= refresh_at)
            && start_all_quota_watch_refresh(
                &mut refresh,
                paths,
                base_url,
                &auth_filter,
                collection_provider_filter,
            )
        {
            next_refresh_at = None;
            continue;
        }

        if event::poll(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS))
            .context("failed to poll quota TUI input")?
        {
            match event::read().context("failed to read quota TUI input")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let command = match key.code {
                        _ if quota_watch_quit_key(key) => Some(QuotaWatchCommand::Quit),
                        KeyCode::Char('j') | KeyCode::Down => Some(QuotaWatchCommand::Down),
                        KeyCode::Char('k') | KeyCode::Up => Some(QuotaWatchCommand::Up),
                        KeyCode::Char('s') => Some(QuotaWatchCommand::Sort),
                        KeyCode::Char('f') => Some(QuotaWatchCommand::Filter),
                        KeyCode::Char('u') | KeyCode::Char('U') => Some(QuotaWatchCommand::Update),
                        _ => None,
                    };
                    let Some(command) = command else {
                        continue;
                    };
                    match apply_quota_watch_command(
                        command,
                        scroll_offset,
                        quota_watch_tui_max_scroll_offset_for_snapshot(
                            &snapshot,
                            detail,
                            provider_filter,
                            sort,
                            quota_watch_tui_table_lines(
                                tui.terminal.size().map(|size| size.height).unwrap_or(24),
                                quota_watch_snapshot_overview_field_count(
                                    &snapshot,
                                    provider_filter,
                                ),
                            ),
                        ),
                    ) {
                        QuotaWatchCommandOutcome::Continue(next_offset) => {
                            if next_offset != scroll_offset {
                                scroll_offset = next_offset;
                                redraw_needed = true;
                            }
                        }
                        QuotaWatchCommandOutcome::Sort => {
                            sort = sort.next();
                            scroll_offset = 0;
                            redraw_needed = true;
                        }
                        QuotaWatchCommandOutcome::Filter => {
                            if !provider_filter_locked {
                                provider_filter = provider_filter.next();
                                scroll_offset = 0;
                                redraw_needed = true;
                            }
                        }
                        QuotaWatchCommandOutcome::Update => {
                            if start_all_quota_watch_refresh(
                                &mut refresh,
                                paths,
                                base_url,
                                &auth_filter,
                                collection_provider_filter,
                            ) {
                                next_refresh_at = None;
                            }
                        }
                        QuotaWatchCommandOutcome::Quit => return Ok(()),
                    }
                }
                Event::Resize(_, _) => {
                    redraw_needed = true;
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn render_all_quota_reports_once_tui(
    frame: &mut ratatui::Frame<'_>,
    reports: &[QuotaReport],
    detail: bool,
) {
    let area = frame.area();
    let snapshot = AllQuotaWatchSnapshot::Reports {
        updated: quota_watch_updated_at(),
        profile_count: reports.len(),
        reports: reports.to_vec(),
    };
    let data = build_all_quota_watch_tui_frame(
        &snapshot,
        AllQuotaWatchLayout {
            detail,
            scroll_offset: 0,
            sort: QuotaReportSort::Remaining,
            provider_filter: QuotaProviderFilter::All,
            provider_filter_locked: false,
            total_width: usize::from(area.width).saturating_sub(4),
            max_lines: quota_watch_tui_table_lines(
                area.height,
                quota_watch_snapshot_overview_field_count(&snapshot, QuotaProviderFilter::All),
            ),
        },
    );
    render_all_quota_watch_tui(frame, &data);
}
