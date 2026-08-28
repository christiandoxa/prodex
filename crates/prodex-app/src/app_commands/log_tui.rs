pub(super) use super::log_throughput::{OutputThroughput, format_output_tokens_per_second};
use crate::{
    AppPaths, AppState, AppStateIoExt, LiveQuotaWatchRuntimeUsageCache,
    RuntimeProfileUsageSnapshot, load_live_quota_watch_runtime_usage_cache,
    load_runtime_usage_snapshots, quota_watch_detail_refresh_interval_for_cached_openai,
};
use chrono::{Local, TimeZone};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prodex_quota::RuntimeQuotaWindowStatus;
use prodex_runtime_doctor::read_runtime_log_tail;
use ratatui::text::{Line, Text};
use std::io;
use std::time::{Duration, Instant};
use terminal_ui::AlternateScreenTerminal;

const LOG_TUI_SHORT_RESET_TIME_FORMAT: &str = "%H:%M";
const LOG_TUI_RESET_TIME_FORMAT: &str = "%m-%d %H:%M";
pub(super) const LOG_TUI_TITLE: &str = "Prodex Log";

pub(super) type LogTuiTerminal = AlternateScreenTerminal<io::Stdout>;

const LOG_TUI_HISTORY_TAIL_BYTES: usize = 1024 * 1024;

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

pub(super) fn render_log_header(
    title: &str,
    count: &str,
    detail: Option<&LogTuiHeaderDetail>,
    throughput_rate: Option<f64>,
    width: usize,
) -> String {
    let inner_width = width.saturating_sub(2);
    let throughput = format_output_tokens_per_second(throughput_rate);
    let throughput_width = terminal_ui::text_width(&throughput);
    if inner_width <= throughput_width {
        return terminal_ui::fit_cell(&throughput, inner_width);
    }

    let left_width = inner_width.saturating_sub(throughput_width + 2);
    let prefix = format!("{title}  {count}");
    let left = if terminal_ui::text_width(&prefix) <= left_width {
        let detail_width = left_width.saturating_sub(terminal_ui::text_width(&prefix) + 2);
        if let Some(detail) = detail.filter(|_| detail_width > 0) {
            format!("{prefix}  {}", detail.render(detail_width))
        } else {
            prefix
        }
    } else {
        terminal_ui::fit_cell(title, left_width)
    };
    let gap = inner_width.saturating_sub(terminal_ui::text_width(&left) + throughput_width);
    format!("{left}{}{}", " ".repeat(gap), throughput)
}

#[derive(Debug, Clone)]
struct LogTuiQuotaDetail {
    five_hour: String,
    weekly: String,
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
                five_hour: format_quota_window(
                    "5h",
                    snapshot.five_hour_status,
                    snapshot.five_hour_remaining_percent,
                    snapshot.five_hour_reset_at,
                    LOG_TUI_SHORT_RESET_TIME_FORMAT,
                ),
                weekly: format_quota_window(
                    "weekly",
                    snapshot.weekly_status,
                    snapshot.weekly_remaining_percent,
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
        let Some(quota) = self.quota.as_ref() else {
            return middle_ellipsize(&self.profile, width);
        };
        let suffix = format!("  {}  {}", quota.five_hour, quota.weekly);
        let profile_width = terminal_ui::text_width(&self.profile);
        let suffix_width = terminal_ui::text_width(&suffix);
        if profile_width + suffix_width <= width {
            return format!("{}{suffix}", self.profile);
        }
        // Header details are semantic segments.  Keep the quota segment atomic and only
        // shorten the profile identity to make room for it.  If even an ellipsis cannot fit,
        // omit the complete quota segment instead of clipping its words into a fake field.
        if suffix_width + 3 <= width {
            return format!(
                "{}{suffix}",
                middle_ellipsize(&self.profile, width - suffix_width)
            );
        }
        if profile_width <= width {
            return self.profile.clone();
        }
        middle_ellipsize(&self.profile, width)
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
    let profile = canonical_header_profile(preferred_profile, state.as_ref())?;
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

fn canonical_header_profile(
    preferred_profile: Option<&str>,
    state: Option<&AppState>,
) -> Option<String> {
    let preferred_profile = preferred_profile
        .map(str::trim)
        .filter(|profile| !profile.is_empty() && *profile != "-");
    if let Some(state) = state {
        if let Some(profile) =
            preferred_profile.filter(|profile| state.profiles.contains_key(*profile))
        {
            return Some(profile.to_string());
        }
        return state
            .active_profile
            .as_deref()
            .filter(|profile| state.profiles.contains_key(*profile))
            .map(ToOwned::to_owned);
    }
    preferred_profile.map(ToOwned::to_owned)
}

pub(super) fn seed_output_throughput_from_history(throughput: &mut OutputThroughput) {
    for path in crate::app_commands::collect_recent_runtime_log_paths(32) {
        let Ok(tail) = read_runtime_log_tail(&path, LOG_TUI_HISTORY_TAIL_BYTES) else {
            continue;
        };
        for line in String::from_utf8_lossy(&tail).lines() {
            if let Some(event) = crate::reports::info_token_usage_event_from_line(line) {
                throughput.observe_historical(&path, &event);
            }
        }
    }
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

fn format_quota_window(
    label: &str,
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    reset_pattern: &str,
) -> String {
    if status == RuntimeQuotaWindowStatus::Unknown {
        return format!("{label} unavailable");
    }
    format!(
        "{label} {} reset {}",
        format_percent(remaining_percent),
        format_snapshot_reset(reset_at, reset_pattern)
    )
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
    fn human_log_modes_share_the_compact_title() {
        assert_eq!(LOG_TUI_TITLE, "Prodex Log");
    }

    #[test]
    fn log_header_keeps_throughput_at_the_right_edge() {
        let detail = LogTuiHeaderDetail::profile_only("main".to_string(), Duration::from_secs(1));
        let header = render_log_header(
            LOG_TUI_TITLE,
            "200 event(s)",
            Some(&detail),
            Some(100.0),
            80,
        );

        assert_eq!(terminal_ui::text_width(&header), 78);
        assert!(header.ends_with("100 t/s"));
        assert!(header.starts_with(LOG_TUI_TITLE));
    }

    #[test]
    fn narrow_log_header_preserves_title_and_idle_marker() {
        let header = render_log_header(LOG_TUI_TITLE, "200 event(s)", None, None, 30);

        assert!(header.contains(LOG_TUI_TITLE));
        assert!(header.ends_with("— t/s"));
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
            plan_type: None,
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
    fn header_detail_does_not_render_unknown_window_as_exhausted() {
        let snapshot = RuntimeProfileUsageSnapshot {
            checked_at: 0,
            plan_type: None,
            five_hour_status: RuntimeQuotaWindowStatus::Unknown,
            five_hour_remaining_percent: 0,
            five_hour_reset_at: i64::MAX,
            weekly_status: RuntimeQuotaWindowStatus::Ready,
            weekly_remaining_percent: 65,
            weekly_reset_at: i64::MAX,
        };

        let detail =
            LogTuiHeaderDetail::quota("main".to_string(), &snapshot, Duration::from_secs(1));
        assert_eq!(
            detail.render(80),
            "main  5h unavailable  weekly 65% reset -"
        );
    }

    #[test]
    fn header_profile_falls_back_to_state_for_untrusted_event_metadata() {
        let state = AppState {
            active_profile: Some("main".to_string()),
            ..AppState::default()
        };

        assert_eq!(canonical_header_profile(Some("second"), Some(&state)), None);
        assert_eq!(
            canonical_header_profile(Some("second"), None),
            Some("second".to_string())
        );
    }

    #[test]
    fn header_detail_right_aligns_output_throughput() {
        let detail = LogTuiHeaderDetail::profile_only("main".to_string(), Duration::from_secs(1));
        let rendered = render_log_header(
            LOG_TUI_TITLE,
            "200 event(s)",
            Some(&detail),
            Some(100.0),
            40,
        );

        assert_eq!(terminal_ui::text_width(&rendered), 38);
        assert!(rendered.ends_with("100 t/s"));
        assert!(rendered.starts_with(LOG_TUI_TITLE));
    }

    #[test]
    fn active_throughput_stays_atomic_across_clipping_boundary() {
        let detail = LogTuiHeaderDetail::profile_only("main".to_string(), Duration::from_secs(1));
        for width in 70..=120 {
            let rendered = render_log_header(
                LOG_TUI_TITLE,
                "200 event(s)",
                Some(&detail),
                Some(100.0),
                width,
            );
            assert!(rendered.contains("100 t/s"), "width {width}: {rendered:?}");
            assert!(
                rendered
                    .strip_suffix(" t/s")
                    .and_then(|prefix| prefix.chars().last())
                    .is_some_and(|character| character.is_ascii_digit())
            );
        }
    }

    #[test]
    fn header_detail_omits_reset_year_and_fits_width() {
        let snapshot = RuntimeProfileUsageSnapshot {
            checked_at: 0,
            plan_type: None,
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
            plan_type: None,
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
