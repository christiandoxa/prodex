use super::*;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::io::IsTerminal;

pub(super) type QuotaWatchTui = terminal_ui::AlternateScreenTerminal<io::Stdout>;

pub(crate) fn quota_watch_enabled(args: &QuotaArgs) -> bool {
    !args.raw && !args.once
}

pub(crate) fn quota_watch_quit_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Esc | KeyCode::Char('q'))
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('z')))
}

pub(crate) fn watch_quota(
    profile_name: &str,
    provider: &ProfileProvider,
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<()> {
    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        match watch_profile_quota_tui(profile_name, provider, codex_home, base_url) {
            Ok(()) => return Ok(()),
            Err(err) if std::env::var_os("PRODEX_TUI_STRICT").is_none() => {
                eprintln!("{}", quota_watch_tui_fallback_message(&err));
            }
            Err(err) => return Err(err),
        }
    }

    loop {
        let output = render_profile_quota_watch_output(
            profile_name,
            &quota_watch_updated_at(),
            fetch_profile_quota(provider, codex_home, base_url)
                .map_err(|err| quota_error_message(&err)),
        );
        print_quota_watch_plain_snapshot(&output)?;
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
}

pub(super) fn print_quota_watch_plain_snapshot(output: &str) -> Result<()> {
    print!("{output}\n\n");
    io::stdout()
        .flush()
        .context("failed to flush quota watch output")?;
    Ok(())
}

fn watch_profile_quota_tui(
    profile_name: &str,
    provider: &ProfileProvider,
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<()> {
    let mut tui = QuotaWatchTui::stdout("quota TUI")?;
    let mut snapshot = ProfileQuotaWatchSnapshot {
        updated: quota_watch_updated_at(),
        quota: Err("Loading quota data...".to_string()),
    };
    let mut refresh = ProfileQuotaWatchRefresh::new();
    let _ = start_profile_quota_watch_refresh(&mut refresh, provider, codex_home, base_url);
    let mut redraw_needed = true;
    let mut next_refresh_at = None;

    loop {
        if let Some(next_snapshot) = refresh.take_latest() {
            snapshot = next_snapshot;
            redraw_needed = true;
            next_refresh_at = Some(quota_watch_next_refresh_at());
        }

        if redraw_needed {
            let frame = build_profile_quota_watch_tui_frame(
                profile_name,
                &snapshot.updated,
                snapshot.quota.clone(),
            );
            tui.terminal
                .draw(|area| render_all_quota_watch_tui(area, &frame))
                .context("failed to draw quota TUI")?;
            redraw_needed = false;
        }

        if next_refresh_at.is_some_and(|refresh_at| Instant::now() >= refresh_at)
            && start_profile_quota_watch_refresh(&mut refresh, provider, codex_home, base_url)
        {
            next_refresh_at = None;
            continue;
        }

        if profile_quota_tui_should_quit(&mut redraw_needed)? {
            return Ok(());
        }
    }
}

fn profile_quota_tui_should_quit(redraw_needed: &mut bool) -> Result<bool> {
    if !event::poll(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS))
        .context("failed to poll quota TUI input")?
    {
        return Ok(false);
    }
    match event::read().context("failed to read quota TUI input")? {
        Event::Key(key) if key.kind == KeyEventKind::Press => Ok(quota_watch_quit_key(key)),
        Event::Resize(_, _) => {
            *redraw_needed = true;
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn start_profile_quota_watch_refresh(
    refresh: &mut ProfileQuotaWatchRefresh,
    provider: &ProfileProvider,
    codex_home: &Path,
    base_url: Option<&str>,
) -> bool {
    let provider = provider.clone();
    let codex_home = codex_home.to_path_buf();
    let base_url = base_url.map(str::to_string);
    refresh.try_start_catching_panic(
        move || ProfileQuotaWatchSnapshot {
            updated: quota_watch_updated_at(),
            quota: fetch_profile_quota(&provider, &codex_home, base_url.as_deref())
                .map_err(|err| quota_error_message(&err)),
        },
        ProfileQuotaWatchSnapshot {
            updated: quota_watch_updated_at(),
            quota: Err("quota refresh failed unexpectedly".to_string()),
        },
    )
}
