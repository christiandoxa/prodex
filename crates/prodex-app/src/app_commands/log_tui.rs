use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::{Line, Text};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
}
