use crate::{
    AppPaths, AppState, AppStateIoExt, RuntimeProfileUsageSnapshot, load_runtime_usage_snapshots,
};
use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use prodex_quota::format_reset_time;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::text::{Line, Text};
use std::io;
use std::time::Duration;

pub(super) const LOG_TUI_HEADER_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

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

pub(super) fn marquee_text(value: &str, width: usize, tick: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let chars = value.chars().chain("   ".chars()).collect::<Vec<_>>();
    if value.chars().count() <= width {
        return value.to_string();
    }
    (0..width)
        .map(|index| chars[(tick + index) % chars.len()])
        .collect()
}

pub(super) fn log_tui_header_detail(preferred_profile: Option<&str>) -> Option<String> {
    let paths = AppPaths::discover().ok();
    let state = paths.as_ref().and_then(|paths| AppState::load(paths).ok());
    let profile = preferred_profile.map(ToOwned::to_owned).or_else(|| {
        state
            .as_ref()
            .and_then(|state| state.active_profile.clone())
    })?;
    let Some((paths, state)) = paths.as_ref().zip(state.as_ref()) else {
        return Some(format!("profile {profile}"));
    };
    let Some(snapshot) = load_runtime_usage_snapshots(paths, &state.profiles)
        .ok()
        .and_then(|snapshots| snapshots.get(&profile).cloned())
    else {
        return Some(format!("profile {profile}"));
    };
    Some(format_log_tui_quota_detail(&profile, &snapshot))
}

fn format_log_tui_quota_detail(profile: &str, snapshot: &RuntimeProfileUsageSnapshot) -> String {
    format!(
        "profile {profile}  5h left {} reset {}  weekly left {} reset {}",
        format_percent(snapshot.five_hour_remaining_percent),
        format_snapshot_reset(snapshot.five_hour_reset_at),
        format_percent(snapshot.weekly_remaining_percent),
        format_snapshot_reset(snapshot.weekly_reset_at)
    )
}

fn format_percent(value: i64) -> String {
    format!("{}%", value.clamp(0, 100))
}

fn format_snapshot_reset(reset_at: i64) -> String {
    format_reset_time((reset_at != i64::MAX).then_some(reset_at))
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
    fn marquee_text_scrolls_long_header_detail() {
        assert_eq!(marquee_text("abcdef", 4, 0), "abcd");
        assert_eq!(marquee_text("abcdef", 4, 2), "cdef");
        assert_eq!(marquee_text("abcdef", 4, 6), "   a");
        assert_eq!(marquee_text("abc", 4, 99), "abc");
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

        assert_eq!(
            format_log_tui_quota_detail("main", &snapshot),
            "profile main  5h left 42% reset -  weekly left 7% reset -"
        );
    }
}
