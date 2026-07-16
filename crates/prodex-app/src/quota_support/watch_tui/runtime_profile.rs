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
            fetch_profile_quota(provider, codex_home, base_url).map_err(|err| err.to_string()),
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
    loop {
        let updated = quota_watch_updated_at();
        let frame = build_profile_quota_watch_tui_frame(
            profile_name,
            &updated,
            fetch_profile_quota(provider, codex_home, base_url).map_err(|err| err.to_string()),
        );
        tui.terminal
            .draw(|area| render_all_quota_watch_tui(area, &frame))
            .context("failed to draw quota TUI")?;

        let refresh_at = quota_watch_next_refresh_at();
        while Instant::now() < refresh_at {
            if event::poll(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS))
                .context("failed to poll quota TUI input")?
            {
                match event::read().context("failed to read quota TUI input")? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if quota_watch_quit_key(key) {
                            return Ok(());
                        }
                    }
                    Event::Resize(_, _) => break,
                    _ => {}
                }
            }
        }
    }
}
