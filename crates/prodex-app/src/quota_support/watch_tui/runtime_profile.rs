use super::*;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::IsTerminal;

pub(super) struct QuotaWatchTui {
    pub(super) terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl QuotaWatchTui {
    pub(super) fn new() -> Result<Self> {
        enable_raw_mode().context("failed to enable quota TUI raw mode")?;
        let mut stdout = io::stdout();
        if let Err(err) = crossterm::execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(err).context("failed to enter quota TUI alternate screen");
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let mut stdout = io::stdout();
                let _ = crossterm::execute!(stdout, Show, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err).context("failed to initialize quota TUI terminal");
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for QuotaWatchTui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

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
                eprintln!("prodex quota TUI unavailable, falling back to plain watch: {err:#}");
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

pub(crate) fn quota_watch_available_report_lines(header: &str) -> Option<usize> {
    let terminal_height = terminal_height_lines()?;
    let reserved = header.lines().count().saturating_add(2);
    Some(terminal_height.saturating_sub(reserved))
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
    let mut tui = QuotaWatchTui::new()?;
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
