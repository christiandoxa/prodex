use crate::{
    AppPaths, AppState, AppStateIoExt, LiveQuotaWatchRuntimeUsageCache,
    RuntimeProfileUsageSnapshot, load_live_quota_watch_runtime_usage_cache,
    load_runtime_usage_snapshots, quota_watch_detail_refresh_interval_for_cached_openai,
};
use anyhow::{Context, Result};
use chrono::{Local, TimeZone};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use prodex_quota::RuntimeQuotaWindowStatus;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::text::{Line, Text};
use std::io;
use std::time::{Duration, Instant};

const LOG_TUI_SHORT_RESET_TIME_FORMAT: &str = "%H:%M";
const LOG_TUI_RESET_TIME_FORMAT: &str = "%m-%d %H:%M";

pub(super) struct LogTuiTerminal {
    pub(super) terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl LogTuiTerminal {
    pub(super) fn new(label: &str) -> Result<Self> {
        enable_raw_mode().with_context(|| format!("failed to enable {label} TUI raw mode"))?;
        let mut stdout = io::stdout();
        if let Err(err) = crossterm::execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(err)
                .with_context(|| format!("failed to enter {label} TUI alternate screen"));
        }
        let terminal = Terminal::new(CrosstermBackend::new(stdout)).inspect_err(|_| {
            let mut stdout = io::stdout();
            let _ = crossterm::execute!(stdout, Show, LeaveAlternateScreen);
            let _ = disable_raw_mode();
        })?;
        Ok(Self { terminal })
    }
}

impl Drop for LogTuiTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

#[derive(Debug, Default, Clone)]
pub(super) struct LogTuiState {
    scroll_from_bottom: usize,
    search: String,
    editing_search: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LogTuiInput {
    Continue,
    Quit,
}

#[derive(Debug, Clone)]
pub(super) struct LogTuiHeaderDetail {
    profile: String,
    quota: Option<LogTuiQuotaDetail>,
    refresh_interval: Duration,
}

#[derive(Debug, Clone)]
struct LogTuiQuotaDetail {
    five_hour_left: String,
    five_hour_reset: String,
    weekly_left: String,
    weekly_reset: String,
}

impl LogTuiHeaderDetail {
    fn profile_only(profile: String, refresh_interval: Duration) -> Self {
        Self {
            profile,
            quota: None,
            refresh_interval,
        }
    }

    fn quota(
        profile: String,
        snapshot: &RuntimeProfileUsageSnapshot,
        refresh_interval: Duration,
    ) -> Self {
        Self {
            profile,
            quota: Some(LogTuiQuotaDetail {
                five_hour_left: format_percent(snapshot.five_hour_remaining_percent),
                five_hour_reset: format_snapshot_reset(
                    snapshot.five_hour_reset_at,
                    LOG_TUI_SHORT_RESET_TIME_FORMAT,
                ),
                weekly_left: format_percent(snapshot.weekly_remaining_percent),
                weekly_reset: format_snapshot_reset(
                    snapshot.weekly_reset_at,
                    LOG_TUI_RESET_TIME_FORMAT,
                ),
            }),
            refresh_interval,
        }
    }

    fn refresh_interval(&self) -> Duration {
        self.refresh_interval
    }

    pub(super) fn render(&self, width: usize) -> String {
        if width == 0 {
            return String::new();
        }
        let suffix = self.quota.as_ref().map(|quota| {
            format!(
                "  5h {} reset {}  weekly {} reset {}",
                quota.five_hour_left, quota.five_hour_reset, quota.weekly_left, quota.weekly_reset
            )
        });
        let suffix_text = suffix.as_deref().unwrap_or("");
        let reserved = terminal_ui::text_width(suffix_text);
        if reserved >= width {
            return terminal_ui::fit_cell(&format!("{}{suffix_text}", self.profile), width);
        }
        let profile = middle_ellipsize(&self.profile, width - reserved);
        format!("{profile}{suffix_text}")
    }
}

pub(super) fn log_tui_header_next_refresh_at(
    detail: Option<&LogTuiHeaderDetail>,
    now: Instant,
) -> Instant {
    now + detail
        .map(LogTuiHeaderDetail::refresh_interval)
        .unwrap_or_else(log_tui_header_missing_refresh_interval)
}

impl LogTuiState {
    pub(super) fn apply_key(&mut self, key: KeyEvent) -> LogTuiInput {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('z'))
        {
            return LogTuiInput::Quit;
        }

        if self.editing_search {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => self.editing_search = false,
                KeyCode::Backspace => {
                    self.search.pop();
                    self.scroll_from_bottom = 0;
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search.clear();
                    self.scroll_from_bottom = 0;
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search.push(ch);
                    self.scroll_from_bottom = 0;
                }
                _ => {}
            }
            return LogTuiInput::Continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => LogTuiInput::Quit,
            KeyCode::Char('/') => {
                self.search.clear();
                self.editing_search = true;
                self.scroll_from_bottom = 0;
                LogTuiInput::Continue
            }
            KeyCode::Char('c') => {
                self.search.clear();
                self.scroll_from_bottom = 0;
                LogTuiInput::Continue
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_from_bottom = self.scroll_from_bottom.saturating_add(1);
                LogTuiInput::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_from_bottom = self.scroll_from_bottom.saturating_sub(1);
                LogTuiInput::Continue
            }
            KeyCode::PageUp => {
                self.scroll_from_bottom = self.scroll_from_bottom.saturating_add(10);
                LogTuiInput::Continue
            }
            KeyCode::PageDown => {
                self.scroll_from_bottom = self.scroll_from_bottom.saturating_sub(10);
                LogTuiInput::Continue
            }
            KeyCode::Home => {
                self.scroll_from_bottom = usize::MAX;
                LogTuiInput::Continue
            }
            KeyCode::End => {
                self.scroll_from_bottom = 0;
                LogTuiInput::Continue
            }
            _ => LogTuiInput::Continue,
        }
    }

    pub(super) fn query(&self) -> Option<&str> {
        let query = self.search.trim();
        (!query.is_empty()).then_some(query)
    }

    pub(super) fn scroll_from_bottom(&self) -> usize {
        self.scroll_from_bottom
    }

    pub(super) fn footer_text(&self, prefix: &str) -> String {
        let search = if self.editing_search {
            format!(" | search: /{}_", self.search)
        } else if let Some(query) = self.query() {
            format!(" | search: /{query} (c clear)")
        } else {
            " | / search".to_string()
        };
        format!("{prefix} | ↑/↓ scroll PgUp/PgDn Home/End{search}")
    }
}

pub(super) fn visible_text(
    mut lines: Vec<Line<'static>>,
    max_lines: usize,
    scroll_from_bottom: usize,
) -> Text<'static> {
    if max_lines == 0 {
        lines.clear();
        return Text::from(lines);
    }
    let hidden = lines.len().saturating_sub(max_lines);
    let offset = hidden.saturating_sub(scroll_from_bottom.min(hidden));
    let end = offset.saturating_add(max_lines).min(lines.len());
    Text::from(lines.drain(offset..end).collect::<Vec<_>>())
}

pub(super) fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

pub(super) fn log_tui_header_detail(preferred_profile: Option<&str>) -> Option<LogTuiHeaderDetail> {
    let paths = AppPaths::discover().ok();
    let state = paths.as_ref().and_then(|paths| AppState::load(paths).ok());
    let profile = preferred_profile.map(ToOwned::to_owned).or_else(|| {
        state
            .as_ref()
            .and_then(|state| state.active_profile.clone())
    })?;
    let Some((paths, state)) = paths.as_ref().zip(state.as_ref()) else {
        return Some(log_tui_header_profile_only_detail(profile));
    };
    let now = Local::now().timestamp();
    let quota_watch_cache = load_live_quota_watch_runtime_usage_cache(paths, &state.profiles, now);
    if let Some(cache) = quota_watch_cache.as_ref()
        && let Some(snapshot) = cache.snapshots.get(&profile)
    {
        return Some(LogTuiHeaderDetail::quota(
            profile,
            snapshot,
            cache.refresh_interval_at(now),
        ));
    }
    let Ok(snapshots) = load_runtime_usage_snapshots(paths, &state.profiles) else {
        return Some(log_tui_header_profile_only_detail_with_interval(
            profile,
            log_tui_header_cache_or_missing_refresh_interval(quota_watch_cache.as_ref(), now),
        ));
    };
    let Some(snapshot) = snapshots.get(&profile) else {
        return Some(log_tui_header_profile_only_detail_with_interval(
            profile,
            log_tui_header_cache_or_missing_refresh_interval(quota_watch_cache.as_ref(), now),
        ));
    };
    Some(LogTuiHeaderDetail::quota(
        profile,
        snapshot,
        quota_watch_cache
            .as_ref()
            .map(|cache| cache.refresh_interval_at(now))
            .unwrap_or_else(|| {
                log_tui_header_snapshot_refresh_interval(snapshot, state.profiles.len(), now)
            }),
    ))
}

fn log_tui_header_profile_only_detail(profile: String) -> LogTuiHeaderDetail {
    log_tui_header_profile_only_detail_with_interval(
        profile,
        log_tui_header_missing_refresh_interval(),
    )
}

fn log_tui_header_profile_only_detail_with_interval(
    profile: String,
    refresh_interval: Duration,
) -> LogTuiHeaderDetail {
    LogTuiHeaderDetail::profile_only(profile, refresh_interval)
}

fn log_tui_header_cache_or_missing_refresh_interval(
    cache: Option<&LiveQuotaWatchRuntimeUsageCache>,
    now: i64,
) -> Duration {
    cache
        .map(|cache| cache.refresh_interval_at(now))
        .unwrap_or_else(log_tui_header_missing_refresh_interval)
}

fn log_tui_header_missing_refresh_interval() -> Duration {
    quota_watch_detail_refresh_interval_for_cached_openai(&[], true, 1, Local::now().timestamp())
}

fn log_tui_header_snapshot_refresh_interval(
    snapshot: &RuntimeProfileUsageSnapshot,
    profile_count: usize,
    now: i64,
) -> Duration {
    let watch = matches!(
        snapshot.five_hour_status,
        RuntimeQuotaWindowStatus::Exhausted | RuntimeQuotaWindowStatus::Unknown
    ) || matches!(
        snapshot.weekly_status,
        RuntimeQuotaWindowStatus::Exhausted | RuntimeQuotaWindowStatus::Unknown
    );
    let reset_windows = [snapshot.five_hour_reset_at, snapshot.weekly_reset_at]
        .into_iter()
        .filter(|reset_at| *reset_at != i64::MAX)
        .collect::<Vec<_>>();
    quota_watch_detail_refresh_interval_for_cached_openai(
        &reset_windows,
        watch,
        profile_count.max(1),
        now,
    )
}

fn format_percent(value: i64) -> String {
    format!("{}%", value.clamp(0, 100))
}

fn format_snapshot_reset(reset_at: i64, pattern: &str) -> String {
    if reset_at == i64::MAX {
        return "-".to_string();
    }
    Local
        .timestamp_opt(reset_at, 0)
        .single()
        .map(|dt| dt.format(pattern).to_string())
        .unwrap_or_else(|| reset_at.to_string())
}

fn middle_ellipsize(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if terminal_ui::text_width(value) <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let mut left = String::new();
    let mut left_width = 0;
    let left_limit = (width - 3) / 2;
    for ch in value.chars() {
        let ch_width = terminal_ui::text_width(&ch.to_string());
        if left_width + ch_width > left_limit {
            break;
        }
        left.push(ch);
        left_width += ch_width;
    }

    let mut right = String::new();
    let mut right_width = 0;
    let right_limit = width - 3 - left_width;
    for ch in value.chars().rev() {
        let ch_width = terminal_ui::text_width(&ch.to_string());
        if right_width + ch_width > right_limit {
            break;
        }
        right.insert(0, ch);
        right_width += ch_width;
    }
    format!("{left}...{right}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use prodex_quota::RuntimeQuotaWindowStatus;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn maps_scroll_and_search_keys() {
        let mut state = LogTuiState::default();

        assert_eq!(state.apply_key(key(KeyCode::Up)), LogTuiInput::Continue);
        assert_eq!(state.scroll_from_bottom(), 1);
        assert_eq!(state.apply_key(key(KeyCode::Down)), LogTuiInput::Continue);
        assert_eq!(state.scroll_from_bottom(), 0);

        state.apply_key(key(KeyCode::Char('/')));
        state.apply_key(key(KeyCode::Char('h')));
        state.apply_key(key(KeyCode::Char('i')));
        state.apply_key(key(KeyCode::Enter));

        assert_eq!(state.query(), Some("hi"));
        assert!(state.footer_text("q quit").contains("search: /hi"));
    }

    #[test]
    fn slices_visible_text_from_bottom_with_scroll_offset() {
        let lines = (0..5)
            .map(|index| Line::raw(format!("line {index}")))
            .collect();

        let text = visible_text(lines, 2, 1);
        let rendered = text
            .lines
            .iter()
            .map(|line| line.spans[0].content.as_ref())
            .collect::<Vec<_>>();

        assert_eq!(rendered, ["line 2", "line 3"]);
    }

    #[test]
    fn middle_ellipsizes_long_header_profile() {
        assert_eq!(middle_ellipsize("abcdefghijkl", 8), "ab...jkl");
        assert_eq!(middle_ellipsize("abc", 8), "abc");
        assert_eq!(middle_ellipsize("abcdef", 3), "...");
    }

    #[test]
    fn formats_header_profile_quota_detail() {
        let snapshot = RuntimeProfileUsageSnapshot {
            checked_at: 0,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 42,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Critical,
            weekly_remaining_percent: 7,
            weekly_reset_at: i64::MAX,
        };

        let detail =
            LogTuiHeaderDetail::quota("main".to_string(), &snapshot, Duration::from_secs(1));
        assert_eq!(detail.render(80), "main  5h 42% reset -  weekly 7% reset -");
    }

    #[test]
    fn header_detail_omits_reset_year_and_fits_width() {
        let snapshot = RuntimeProfileUsageSnapshot {
            checked_at: 0,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 42,
            five_hour_reset_at: 1_783_523_600,
            weekly_status: RuntimeQuotaWindowStatus::Critical,
            weekly_remaining_percent: 7,
            weekly_reset_at: 1_783_527_200,
        };
        let detail = LogTuiHeaderDetail::quota(
            "very_long_profile_name_example.com".to_string(),
            &snapshot,
            Duration::from_secs(1),
        );
        let rendered = detail.render(72);

        assert!(terminal_ui::text_width(&rendered) <= 72);
        assert!(rendered.contains("..."));
        assert!(!rendered.contains("2026-"));
        assert!(!rendered.contains(" left "));
    }

    #[test]
    fn header_refresh_interval_uses_quota_detail_algorithm() {
        let snapshot = RuntimeProfileUsageSnapshot {
            checked_at: 0,
            five_hour_status: RuntimeQuotaWindowStatus::Ready,
            five_hour_remaining_percent: 80,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 80,
            weekly_reset_at: i64::MAX,
        };

        assert_eq!(
            log_tui_header_snapshot_refresh_interval(&snapshot, 1, 0),
            Duration::from_secs(41)
        );
        assert_eq!(
            log_tui_header_snapshot_refresh_interval(
                &RuntimeProfileUsageSnapshot {
                    five_hour_reset_at: 15 * 60,
                    ..snapshot.clone()
                },
                1,
                0
            ),
            Duration::from_secs(9)
        );
        assert_eq!(
            log_tui_header_snapshot_refresh_interval(
                &RuntimeProfileUsageSnapshot {
                    five_hour_reset_at: 2 * 60,
                    ..snapshot
                },
                1,
                0
            ),
            Duration::from_secs(5)
        );
    }
}
