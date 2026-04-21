use super::*;

#[derive(Debug)]
enum AllQuotaWatchSnapshot {
    Reports {
        updated: String,
        profile_count: usize,
        reports: Vec<QuotaReport>,
    },
    Empty {
        updated: String,
    },
    Error {
        updated: String,
        message: String,
    },
}

const QUOTA_WATCH_INPUT_POLL_MS: u64 = 100;

enum QuotaWatchCommand {
    Up,
    Down,
    Quit,
}

enum QuotaWatchCommandOutcome {
    Continue(usize),
    Quit,
}

struct QuotaWatchInput {
    tty: fs::File,
    saved_stty_state: String,
}

impl QuotaWatchInput {
    fn open() -> Option<Self> {
        let tty = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        let saved_stty_state = quota_watch_stty_output(&tty, &["-g"])?.trim().to_string();
        quota_watch_stty_apply(&tty, &["-icanon", "-echo", "min", "0", "time", "1"])?;
        Some(Self {
            tty,
            saved_stty_state,
        })
    }

    fn read_command(&mut self) -> Option<QuotaWatchCommand> {
        let mut buf = [0_u8; 8];
        let bytes_read = self.tty.read(&mut buf).ok()?;
        if bytes_read == 0 {
            return None;
        }
        let bytes = &buf[..bytes_read];
        match bytes {
            [b'q' | b'Q', ..] => Some(QuotaWatchCommand::Quit),
            [b'k' | b'K', ..] => Some(QuotaWatchCommand::Up),
            [b'j' | b'J', ..] => Some(QuotaWatchCommand::Down),
            [0x1b, b'[', b'A', ..] => Some(QuotaWatchCommand::Up),
            [0x1b, b'[', b'B', ..] => Some(QuotaWatchCommand::Down),
            _ => None,
        }
    }
}

impl Drop for QuotaWatchInput {
    fn drop(&mut self) {
        let _ = quota_watch_stty_apply(&self.tty, &[self.saved_stty_state.as_str()]);
    }
}

fn quota_watch_stty_output(tty: &fs::File, args: &[&str]) -> Option<String> {
    let output = Command::new("stty")
        .args(args)
        .stdin(tty.try_clone().ok()?)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
}

fn quota_watch_stty_apply(tty: &fs::File, args: &[&str]) -> Option<()> {
    let output = Command::new("stty")
        .args(args)
        .stdin(tty.try_clone().ok()?)
        .output()
        .ok()?;
    output.status.success().then_some(())
}

pub(crate) fn quota_watch_enabled(args: &QuotaArgs) -> bool {
    !args.raw && !args.once
}

pub(crate) fn render_profile_quota_watch_output(
    profile_name: &str,
    _updated: &str,
    quota_result: std::result::Result<ProviderQuotaSnapshot, String>,
) -> String {
    match quota_result {
        Ok(quota) => render_profile_quota_snapshot(profile_name, &quota),
        Err(err) => render_quota_watch_error_panel(&format!("Quota {profile_name}"), &err),
    }
}

#[allow(dead_code)]
pub(crate) fn render_all_quota_watch_output(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
    detail: bool,
) -> String {
    render_all_quota_watch_snapshot(
        &collect_all_quota_watch_snapshot(updated, state_result, base_url),
        detail,
        0,
    )
}

fn collect_all_quota_watch_snapshot(
    updated: &str,
    state_result: std::result::Result<AppState, String>,
    base_url: Option<&str>,
) -> AllQuotaWatchSnapshot {
    match state_result {
        Ok(state) if !state.profiles.is_empty() => AllQuotaWatchSnapshot::Reports {
            updated: updated.to_string(),
            profile_count: state.profiles.len(),
            reports: collect_quota_reports(&state, base_url),
        },
        Ok(_) => AllQuotaWatchSnapshot::Empty {
            updated: updated.to_string(),
        },
        Err(err) => AllQuotaWatchSnapshot::Error {
            updated: updated.to_string(),
            message: err,
        },
    }
}

fn render_all_quota_watch_snapshot(
    snapshot: &AllQuotaWatchSnapshot,
    detail: bool,
    scroll_offset: usize,
) -> String {
    match snapshot {
        AllQuotaWatchSnapshot::Reports {
            updated: _updated,
            profile_count: _profile_count,
            reports,
        } => render_all_quota_watch_report_output(reports, detail, scroll_offset),
        AllQuotaWatchSnapshot::Empty { updated: _updated } => {
            render_quota_watch_error_panel("Quota", "No profiles configured")
        }
        AllQuotaWatchSnapshot::Error {
            updated: _updated,
            message,
        } => render_quota_watch_error_panel("Quota", message),
    }
}

fn render_all_quota_watch_report_output(
    reports: &[QuotaReport],
    detail: bool,
    scroll_offset: usize,
) -> String {
    render_quota_reports_window_with_layout(
        reports,
        detail,
        quota_watch_available_report_lines(""),
        current_cli_width(),
        scroll_offset,
        true,
    )
    .output
}

fn redraw_quota_watch(output: &str) -> Result<()> {
    print!("\x1b[H\x1b[2J{output}");
    io::stdout()
        .flush()
        .context("failed to flush quota watch output")?;
    Ok(())
}

fn quota_watch_available_report_lines(header: &str) -> Option<usize> {
    let terminal_height = terminal_height_lines()?;
    let reserved = header.lines().count().saturating_add(1);
    Some(terminal_height.saturating_sub(reserved))
}

pub(crate) fn watch_quota(
    profile_name: &str,
    provider: &ProfileProvider,
    codex_home: &Path,
    base_url: Option<&str>,
) -> Result<()> {
    loop {
        let output = render_profile_quota_watch_output(
            profile_name,
            &quota_watch_updated_at(),
            fetch_profile_quota(provider, codex_home, base_url).map_err(|err| err.to_string()),
        );
        redraw_quota_watch(&output)?;
        thread::sleep(Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS));
    }
}

pub(crate) fn watch_all_quotas(
    paths: &AppPaths,
    base_url: Option<&str>,
    detail: bool,
) -> Result<()> {
    let mut input = QuotaWatchInput::open();
    let mut scroll_offset = 0_usize;
    let mut snapshot = load_all_quota_watch_snapshot(paths, base_url);
    let mut redraw_needed = true;
    let mut next_refresh_at = quota_watch_next_refresh_at();

    loop {
        if redraw_needed {
            scroll_offset = scroll_offset.min(quota_watch_max_scroll_offset(&snapshot));
            let output = render_all_quota_watch_snapshot(&snapshot, detail, scroll_offset);
            redraw_quota_watch(&output)?;
            redraw_needed = false;
        }

        if Instant::now() >= next_refresh_at {
            snapshot = load_all_quota_watch_snapshot(paths, base_url);
            redraw_needed = true;
            next_refresh_at = quota_watch_next_refresh_at();
            continue;
        }

        if let Some(command) = input.as_mut().and_then(QuotaWatchInput::read_command) {
            match apply_quota_watch_command(
                command,
                scroll_offset,
                quota_watch_max_scroll_offset(&snapshot),
            ) {
                QuotaWatchCommandOutcome::Continue(next_offset) => {
                    if next_offset != scroll_offset {
                        scroll_offset = next_offset;
                        redraw_needed = true;
                    }
                }
                QuotaWatchCommandOutcome::Quit => return Ok(()),
            }
        }

        thread::sleep(Duration::from_millis(QUOTA_WATCH_INPUT_POLL_MS));
    }
}

fn quota_watch_updated_at() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string()
}

fn load_all_quota_watch_snapshot(
    paths: &AppPaths,
    base_url: Option<&str>,
) -> AllQuotaWatchSnapshot {
    collect_all_quota_watch_snapshot(
        &quota_watch_updated_at(),
        AppState::load(paths).map_err(|err| err.to_string()),
        base_url,
    )
}

fn quota_watch_next_refresh_at() -> Instant {
    Instant::now() + Duration::from_secs(DEFAULT_WATCH_INTERVAL_SECONDS)
}

fn quota_watch_max_scroll_offset(snapshot: &AllQuotaWatchSnapshot) -> usize {
    match snapshot {
        AllQuotaWatchSnapshot::Reports { reports, .. } => reports.len().saturating_sub(1),
        _ => 0,
    }
}

fn apply_quota_watch_command(
    command: QuotaWatchCommand,
    scroll_offset: usize,
    max_scroll_offset: usize,
) -> QuotaWatchCommandOutcome {
    match command {
        QuotaWatchCommand::Up => {
            QuotaWatchCommandOutcome::Continue(scroll_offset.saturating_sub(1))
        }
        QuotaWatchCommand::Down => QuotaWatchCommandOutcome::Continue(
            scroll_offset.saturating_add(1).min(max_scroll_offset),
        ),
        QuotaWatchCommand::Quit => QuotaWatchCommandOutcome::Quit,
    }
}
