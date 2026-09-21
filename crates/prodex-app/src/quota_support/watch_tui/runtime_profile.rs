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
    detail: bool,
    base_url: Option<&str>,
) -> Result<()> {
    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        match watch_profile_quota_tui(profile_name, provider, codex_home, detail, base_url) {
            Ok(()) => return Ok(()),
            Err(err) if std::env::var_os("PRODEX_TUI_STRICT").is_none() => {
                eprintln!("{}", quota_watch_tui_fallback_message(&err));
            }
            Err(err) => return Err(err),
        }
    }

    let mut snapshot = ProfileQuotaWatchSnapshot {
        updated: quota_watch_updated_at(),
        quota: Err("Loading quota data...".to_string()),
    };
    loop {
        let next_snapshot = ProfileQuotaWatchSnapshot {
            updated: quota_watch_updated_at(),
            quota: fetch_profile_quota(provider, codex_home, base_url)
                .map_err(|err| quota_error_message(&err)),
        };
        let output = render_profile_quota_watch_plain_snapshot(
            &mut snapshot,
            next_snapshot,
            profile_name,
            detail,
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

fn render_profile_quota_watch_plain_snapshot(
    previous: &mut ProfileQuotaWatchSnapshot,
    next: ProfileQuotaWatchSnapshot,
    profile_name: &str,
    detail: bool,
) -> String {
    let merged = merge_profile_quota_watch_snapshot(previous, next);
    *previous = merged;
    render_profile_quota_watch_output(
        profile_name,
        &previous.updated,
        previous.quota.clone(),
        detail,
    )
}

fn watch_profile_quota_tui(
    profile_name: &str,
    provider: &ProfileProvider,
    codex_home: &Path,
    detail: bool,
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
            snapshot = merge_profile_quota_watch_snapshot(&snapshot, next_snapshot);
            redraw_needed = true;
            next_refresh_at = Some(quota_watch_next_refresh_at());
        }

        if redraw_needed {
            let frame = build_profile_quota_watch_tui_frame(
                profile_name,
                &snapshot.updated,
                snapshot.quota.clone(),
                detail,
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

fn merge_profile_quota_watch_snapshot(
    previous: &ProfileQuotaWatchSnapshot,
    next: ProfileQuotaWatchSnapshot,
) -> ProfileQuotaWatchSnapshot {
    if previous.quota.is_ok() && next.quota.is_err() {
        previous.clone()
    } else {
        next
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_quota_watch_keeps_previous_success_on_transient_refresh_error() {
        let merged = merge_profile_quota_watch_snapshot(
            &ProfileQuotaWatchSnapshot {
                updated: "before".to_string(),
                quota: Ok(ProviderQuotaSnapshot::OpenAi(UsageResponse {
                    email: Some("before@example.com".to_string()),
                    plan_type: Some("plus".to_string()),
                    rate_limit: None,
                    code_review_rate_limit: None,
                    rate_limit_reset_credits: None,
                    additional_rate_limits: Vec::new(),
                })),
            },
            ProfileQuotaWatchSnapshot {
                updated: "after".to_string(),
                quota: Err("HTTP 503".to_string()),
            },
        );

        let ProfileQuotaWatchSnapshot { updated, quota } = merged;
        assert_eq!(updated, "before");
        let Ok(ProviderQuotaSnapshot::OpenAi(usage)) = quota else {
            panic!("expected previous successful quota snapshot");
        };
        assert_eq!(usage.email.as_deref(), Some("before@example.com"));
    }

    #[test]
    fn profile_quota_watch_plain_output_keeps_previous_success_on_refresh_error() {
        let mut snapshot = ProfileQuotaWatchSnapshot {
            updated: "before".to_string(),
            quota: Ok(ProviderQuotaSnapshot::OpenAi(UsageResponse {
                email: Some("before@example.com".to_string()),
                plan_type: Some("plus".to_string()),
                rate_limit: None,
                code_review_rate_limit: None,
                rate_limit_reset_credits: None,
                additional_rate_limits: Vec::new(),
            })),
        };

        let output = render_profile_quota_watch_plain_snapshot(
            &mut snapshot,
            ProfileQuotaWatchSnapshot {
                updated: "after".to_string(),
                quota: Err("HTTP 503".to_string()),
            },
            "main",
            false,
        );

        assert!(output.contains("before@example.com"));
        assert!(!output.contains("HTTP 503"));
        assert_eq!(snapshot.updated, "before");
    }
}
