use super::LoginMethod;
use anyhow::{Context, Result, bail};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
#[cfg(test)]
use std::io::Read;
use std::io::{self, IsTerminal, Write};

const LOGIN_MENU_MIN_VISIBLE_ITEMS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoginMenuAction {
    Method(LoginMethod),
    Guidance(LoginGuidanceKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoginGuidanceKind {
    GeminiApiKey,
    AnthropicApiKey,
    DeepSeekApiKey,
    CopilotImport,
}

#[derive(Debug, Clone, Copy)]
struct LoginMenuEntry {
    title: &'static str,
    provider: &'static str,
    auth: &'static str,
    usage: &'static str,
    command: &'static str,
    action: LoginMenuAction,
}

#[derive(Debug, Clone, Copy)]
struct LoginMenuLayout {
    visible_items: usize,
    compact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginMenuKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Cancel,
    Digit(usize),
    Ignore,
}

struct LoginMenuTui {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
}

impl LoginMenuTui {
    fn new() -> Result<Self> {
        enable_raw_mode().context("failed to enable login TUI raw mode")?;
        let mut stderr = io::stderr();
        if let Err(err) = crossterm::execute!(stderr, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(err).context("failed to enter login TUI alternate screen");
        }
        let backend = CrosstermBackend::new(stderr);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let mut stderr = io::stderr();
                let _ = crossterm::execute!(stderr, Show, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err).context("failed to initialize login TUI terminal");
            }
        };
        Ok(Self { terminal })
    }
}

impl Drop for LoginMenuTui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub(super) fn login_prompt_is_interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

fn login_menu_entries() -> &'static [LoginMenuEntry] {
    const ENTRIES: &[LoginMenuEntry] = &[
        LoginMenuEntry {
            title: "Sign in with ChatGPT (OpenAI OAuth)",
            provider: "OpenAI / Codex",
            auth: "ChatGPT OAuth",
            usage: "Default quota-aware profile pool for prodex run, prodex s, and prodex caveman.",
            command: "prodex login",
            action: LoginMenuAction::Method(LoginMethod::ChatGpt),
        },
        LoginMenuEntry {
            title: "OpenAI device code",
            provider: "OpenAI / Codex",
            auth: "Device-code OAuth",
            usage: "Same OpenAI/Codex profile type, useful on a terminal without a local browser.",
            command: "prodex login --device-auth",
            action: LoginMenuAction::Method(LoginMethod::DeviceCode),
        },
        LoginMenuEntry {
            title: "Provide your own API key (OpenAI/API-compatible)",
            provider: "OpenAI / local / OpenAI-compatible endpoint",
            auth: "API key stored in the selected Prodex profile",
            usage: "Use OpenAI API billing, or provide a compatible base URL for local/custom endpoints.",
            command: "prodex login --with-api-key [--base-url URL]",
            action: LoginMenuAction::Method(LoginMethod::ApiKey),
        },
        LoginMenuEntry {
            title: "Google Gemini OAuth",
            provider: "Google Gemini",
            auth: "Google OAuth profile",
            usage: "Reusable Gemini profile for prodex s --provider gemini, including OAuth rotation.",
            command: "prodex login --with-google",
            action: LoginMenuAction::Method(LoginMethod::Google),
        },
        LoginMenuEntry {
            title: "Google Gemini API key",
            provider: "Google Gemini",
            auth: "Runtime API key only",
            usage: "Not a persisted login profile; pass GEMINI_API_KEY, GEMINI_API_KEYS, GOOGLE_API_KEY(S), or --api-key when launching Gemini.",
            command: "GEMINI_API_KEY=... prodex s --provider gemini --model gemini-2.5-pro",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::GeminiApiKey),
        },
        LoginMenuEntry {
            title: "Anthropic Claude OAuth",
            provider: "Anthropic Claude",
            auth: "Claude Code OAuth profile",
            usage: "Reusable Anthropic profile for prodex s --provider anthropic without API-key storage.",
            command: "prodex login --with-claude",
            action: LoginMenuAction::Method(LoginMethod::Claude),
        },
        LoginMenuEntry {
            title: "Google Antigravity CLI",
            provider: "Google Antigravity",
            auth: "Antigravity CLI keyring / Google Sign-In",
            usage: "Authenticate the native agy CLI used by prodex s gemini --cli agy.",
            command: "prodex login --with-antigravity",
            action: LoginMenuAction::Method(LoginMethod::Antigravity),
        },
        LoginMenuEntry {
            title: "Anthropic API key",
            provider: "Anthropic Claude",
            auth: "Runtime API key only",
            usage: "Not a persisted login profile; pass ANTHROPIC_API_KEY(S) or --api-key.",
            command: "ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::AnthropicApiKey),
        },
        LoginMenuEntry {
            title: "DeepSeek API key",
            provider: "DeepSeek",
            auth: "Runtime API key only",
            usage: "DeepSeek has no OAuth login in Prodex; use an API key for the provider adapter.",
            command: "DEEPSEEK_API_KEY=... prodex s --provider deepseek --model deepseek-v4-pro",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::DeepSeekApiKey),
        },
        LoginMenuEntry {
            title: "GitHub Copilot import",
            provider: "GitHub Copilot",
            auth: "Existing Copilot CLI account import",
            usage: "Record the Copilot identity from local Copilot CLI state, then launch with --provider copilot.",
            command: "prodex profile import copilot",
            action: LoginMenuAction::Guidance(LoginGuidanceKind::CopilotImport),
        },
    ];
    ENTRIES
}

pub(super) fn prompt_login_menu_action() -> Result<LoginMenuAction> {
    if let Some(action) = prompt_login_menu_action_raw()? {
        return Ok(action);
    }
    prompt_login_menu_action_numbered()
}

fn prompt_login_menu_action_raw() -> Result<Option<LoginMenuAction>> {
    let entries = login_menu_entries();
    let mut tui = match LoginMenuTui::new() {
        Ok(tui) => tui,
        Err(_) => return Ok(None),
    };
    let mut selected = 0usize;
    let mut offset = 0usize;

    loop {
        let size = tui
            .terminal
            .size()
            .context("failed to read login TUI terminal size")?;
        let layout = login_menu_layout_for_rows(usize::from(size.height), entries.len());
        offset = login_menu_window_offset(selected, offset, layout.visible_items, entries.len());
        tui.terminal
            .draw(|frame| render_login_menu_tui(frame, entries, selected, offset, layout))
            .context("failed to draw login TUI")?;
        let key = loop {
            match event::read().context("failed to read login TUI input")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    break login_menu_key_from_event(key);
                }
                Event::Resize(_, _) => {
                    break LoginMenuKey::Ignore;
                }
                _ => {}
            }
        };
        match key {
            LoginMenuKey::Up => {
                selected = selected.saturating_sub(1);
            }
            LoginMenuKey::Down => {
                selected = (selected + 1).min(entries.len().saturating_sub(1));
            }
            LoginMenuKey::PageUp => {
                let step = layout.visible_items.saturating_sub(1).max(1);
                selected = selected.saturating_sub(step);
            }
            LoginMenuKey::PageDown => {
                let step = layout.visible_items.saturating_sub(1).max(1);
                selected = (selected + step).min(entries.len().saturating_sub(1));
            }
            LoginMenuKey::Home => {
                selected = 0;
            }
            LoginMenuKey::End => {
                selected = entries.len().saturating_sub(1);
            }
            LoginMenuKey::Enter => return Ok(Some(entries[selected].action)),
            LoginMenuKey::Digit(index) => {
                if let Some(entry) = entries.get(index.saturating_sub(1)) {
                    return Ok(Some(entry.action));
                }
            }
            LoginMenuKey::Cancel => bail!("login cancelled"),
            LoginMenuKey::Ignore => {}
        }
    }
}

fn login_menu_key_from_event(key: KeyEvent) -> LoginMenuKey {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => LoginMenuKey::Up,
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => LoginMenuKey::Down,
        KeyCode::PageUp | KeyCode::Char('u') | KeyCode::Char('U') => LoginMenuKey::PageUp,
        KeyCode::PageDown | KeyCode::Char('d') | KeyCode::Char('D') => LoginMenuKey::PageDown,
        KeyCode::Home | KeyCode::Char('g') => LoginMenuKey::Home,
        KeyCode::End | KeyCode::Char('G') => LoginMenuKey::End,
        KeyCode::Enter => LoginMenuKey::Enter,
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => LoginMenuKey::Cancel,
        KeyCode::Char(ch) if ('1'..='9').contains(&ch) => {
            LoginMenuKey::Digit(ch.to_digit(10).unwrap_or(0) as usize)
        }
        _ => LoginMenuKey::Ignore,
    }
}

fn prompt_login_menu_action_numbered() -> Result<LoginMenuAction> {
    let entries = login_menu_entries();
    let mut stderr = io::stderr();
    writeln!(stderr, "Choose login method:")?;
    for (index, entry) in entries.iter().enumerate() {
        writeln!(stderr, "  {}. {}", index + 1, entry.title)?;
        writeln!(stderr, "     Provider: {}", entry.provider)?;
        writeln!(stderr, "     Auth: {}", entry.auth)?;
        writeln!(stderr, "     Use: {}", entry.usage)?;
        writeln!(stderr, "     Command: {}", entry.command)?;
    }
    loop {
        write!(stderr, "Select login method [1]: ")?;
        stderr.flush()?;
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read login method")?;
        let selected = input.trim();
        if selected.is_empty() {
            return Ok(entries[0].action);
        }
        if let Ok(index) = selected.parse::<usize>()
            && (1..=entries.len()).contains(&index)
        {
            return Ok(entries[index - 1].action);
        }
        writeln!(stderr, "Enter 1 through {}.", entries.len())?;
    }
}

pub(super) fn show_login_guidance(kind: LoginGuidanceKind) -> Result<()> {
    let entry = login_menu_entries()
        .iter()
        .find(|entry| entry.action == LoginMenuAction::Guidance(kind))
        .context("login guidance entry is missing")?;
    let mut stderr = io::stderr();
    writeln!(stderr)?;
    writeln!(stderr, "Provider guidance: {}", entry.title)?;
    writeln!(stderr, "  Provider: {}", entry.provider)?;
    writeln!(stderr, "  Auth: {}", entry.auth)?;
    writeln!(stderr, "  Use: {}", entry.usage)?;
    writeln!(stderr, "  Command: {}", entry.command)?;
    writeln!(
        stderr,
        "  Note: this path is selected at runtime, not stored by prodex login."
    )?;
    writeln!(stderr)?;
    write!(
        stderr,
        "Press Enter to return to login methods, or Ctrl-C to exit."
    )?;
    stderr.flush()?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read login guidance acknowledgement")?;
    Ok(())
}

fn login_menu_layout_for_rows(rows: usize, entry_count: usize) -> LoginMenuLayout {
    if entry_count == 0 {
        return LoginMenuLayout {
            visible_items: 0,
            compact: true,
        };
    }

    let rows = rows.max(8);
    let compact = rows < 16;
    let reserved_rows = if compact { 5 } else { 7 };
    let min_visible = LOGIN_MENU_MIN_VISIBLE_ITEMS.min(entry_count);
    let visible_items = rows
        .saturating_sub(reserved_rows)
        .max(min_visible)
        .min(entry_count);
    LoginMenuLayout {
        visible_items,
        compact,
    }
}

fn login_menu_window_offset(
    selected: usize,
    current_offset: usize,
    visible_items: usize,
    entry_count: usize,
) -> usize {
    if entry_count == 0 || visible_items == 0 {
        return 0;
    }
    let max_offset = entry_count.saturating_sub(visible_items);
    if selected < current_offset {
        selected.min(max_offset)
    } else if selected >= current_offset.saturating_add(visible_items) {
        selected
            .saturating_add(1)
            .saturating_sub(visible_items)
            .min(max_offset)
    } else {
        current_offset.min(max_offset)
    }
}

fn render_login_menu_tui(
    frame: &mut ratatui::Frame<'_>,
    entries: &[LoginMenuEntry],
    selected: usize,
    offset: usize,
    layout: LoginMenuLayout,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(if layout.compact { 5 } else { 7 }),
        ])
        .split(area);

    let visible_items = layout.visible_items.max(1).min(entries.len().max(1));
    let end = offset.saturating_add(visible_items).min(entries.len());
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "Prodex Login - Select login method",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!(
                "showing {}-{} of {}",
                offset.saturating_add(1).min(entries.len()),
                end,
                entries.len()
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(header, chunks[0]);

    let items = if entries.is_empty() {
        vec![ListItem::new(Line::styled(
            "No login methods available",
            Style::default().fg(Color::Red),
        ))]
    } else {
        entries
            .iter()
            .enumerate()
            .take(end)
            .skip(offset)
            .map(|(index, entry)| {
                let marker = if index == selected {
                    ">"
                } else if index == offset && offset > 0 {
                    "^"
                } else if index + 1 == end && end < entries.len() {
                    "v"
                } else {
                    " "
                };
                let style = if index == selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{marker} {:>2}. ", index + 1), style),
                    Span::styled(entry.title, style),
                    Span::raw(" "),
                    Span::styled(
                        format!("[{}]", entry.auth),
                        Style::default().fg(Color::Cyan),
                    ),
                ]))
            })
            .collect()
    };
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(Color::Blue)),
    );
    frame.render_widget(list, chunks[1]);

    let detail = if entries.is_empty() {
        Text::from(Line::styled(
            "No login methods are registered.",
            Style::default().fg(Color::Red),
        ))
    } else {
        login_menu_detail_text(
            &entries[selected.min(entries.len().saturating_sub(1))],
            layout,
        )
    };
    let detail = Paragraph::new(detail)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(detail, chunks[2]);
}

fn login_menu_detail_text(entry: &LoginMenuEntry, layout: LoginMenuLayout) -> Text<'static> {
    if layout.compact {
        return Text::from(vec![
            Line::from(vec![
                Span::styled("Provider ", Style::default().fg(Color::DarkGray)),
                Span::styled(entry.provider, Style::default().fg(Color::Green)),
                Span::raw(" | "),
                Span::styled(entry.auth, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Use ", Style::default().fg(Color::DarkGray)),
                Span::raw(entry.usage),
            ]),
            Line::from(vec![
                Span::styled("Cmd ", Style::default().fg(Color::DarkGray)),
                Span::styled(entry.command, Style::default().fg(Color::Yellow)),
            ]),
        ]);
    }

    Text::from(vec![
        Line::from(vec![
            Span::styled("Provider ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.provider, Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Auth     ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.auth, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Use      ", Style::default().fg(Color::DarkGray)),
            Span::raw(entry.usage),
        ]),
        Line::from(vec![
            Span::styled("Command  ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.command, Style::default().fg(Color::Yellow)),
        ]),
        Line::from(Span::styled(
            "Up/Down move | PageUp/PageDown scroll | Enter select | 1-9 quick select | q cancel",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )),
    ])
}

#[cfg(test)]
fn render_login_menu(
    entries: &[LoginMenuEntry],
    selected: usize,
    offset: usize,
    layout: LoginMenuLayout,
    width: usize,
) -> Vec<String> {
    if entries.is_empty() {
        return vec![fit_login_menu_line(
            "Prodex login: no login methods available",
            width,
        )];
    }

    let width = width.max(40);
    let selected = selected.min(entries.len().saturating_sub(1));
    let offset = offset.min(entries.len().saturating_sub(1));
    let visible_items = layout.visible_items.max(1).min(entries.len());
    let end = offset.saturating_add(visible_items).min(entries.len());
    let mut lines = Vec::new();
    lines.push(fit_login_menu_line(
        &format!(
            "Select login method - showing {}-{} of {}",
            offset + 1,
            end,
            entries.len()
        ),
        width,
    ));
    lines.push(fit_login_menu_line(
        "Up/Down move, PageUp/PageDown scroll, Enter select, 1-9 quick select, q cancel",
        width,
    ));
    for (index, entry) in entries.iter().enumerate().take(end).skip(offset) {
        let marker = if index == selected {
            ">"
        } else if index == offset && offset > 0 {
            "^"
        } else if index + 1 == end && end < entries.len() {
            "v"
        } else {
            " "
        };
        lines.push(fit_login_menu_line(
            &format!(
                "{marker} {:>2}. {} [{}]",
                index + 1,
                entry.title,
                entry.auth
            ),
            width,
        ));
    }

    let entry = &entries[selected];
    lines.push(String::new());
    if layout.compact {
        lines.push(fit_login_menu_line(
            &format!("{} | {}", entry.provider, entry.auth),
            width,
        ));
        lines.push(fit_login_menu_line(
            &format!("Use/Cmd: {}; {}", entry.usage, entry.command),
            width,
        ));
    } else {
        lines.push(fit_login_menu_line(
            &format!("Provider: {}", entry.provider),
            width,
        ));
        lines.push(fit_login_menu_line(&format!("Auth: {}", entry.auth), width));
        lines.push(fit_login_menu_line(&format!("Use: {}", entry.usage), width));
        lines.push(fit_login_menu_line(
            &format!("Command: {}", entry.command),
            width,
        ));
    }
    lines
}

#[cfg(test)]
fn fit_login_menu_line(value: &str, width: usize) -> String {
    let width = width.max(4);
    let char_count = value.chars().count();
    if char_count <= width {
        return value.to_string();
    }
    let mut output: String = value.chars().take(width.saturating_sub(3)).collect();
    output.push_str("...");
    output
}

#[cfg(test)]
fn read_login_menu_key<R: Read>(reader: &mut R) -> io::Result<LoginMenuKey> {
    let first = loop {
        if let Some(byte) = read_login_menu_byte(reader)? {
            break byte;
        }
    };
    match first {
        b'\r' | b'\n' => Ok(LoginMenuKey::Enter),
        3 | 4 => Ok(LoginMenuKey::Cancel),
        b'q' | b'Q' => Ok(LoginMenuKey::Cancel),
        b'k' | b'K' => Ok(LoginMenuKey::Up),
        b'j' | b'J' => Ok(LoginMenuKey::Down),
        b'u' | b'U' => Ok(LoginMenuKey::PageUp),
        b'd' | b'D' => Ok(LoginMenuKey::PageDown),
        b'g' => Ok(LoginMenuKey::Home),
        b'G' => Ok(LoginMenuKey::End),
        b'1'..=b'9' => Ok(LoginMenuKey::Digit((first - b'0') as usize)),
        27 => read_login_menu_escape_key(reader),
        _ => Ok(LoginMenuKey::Ignore),
    }
}

#[cfg(test)]
fn read_login_menu_escape_key<R: Read>(reader: &mut R) -> io::Result<LoginMenuKey> {
    let Some(first) = read_login_menu_byte(reader)? else {
        return Ok(LoginMenuKey::Cancel);
    };
    if first != b'[' && first != b'O' {
        return Ok(LoginMenuKey::Cancel);
    }
    let Some(second) = read_login_menu_byte(reader)? else {
        return Ok(LoginMenuKey::Cancel);
    };
    match second {
        b'A' => Ok(LoginMenuKey::Up),
        b'B' => Ok(LoginMenuKey::Down),
        b'H' => Ok(LoginMenuKey::Home),
        b'F' => Ok(LoginMenuKey::End),
        b'5' => {
            let _ = read_login_menu_byte(reader)?;
            Ok(LoginMenuKey::PageUp)
        }
        b'6' => {
            let _ = read_login_menu_byte(reader)?;
            Ok(LoginMenuKey::PageDown)
        }
        b'1' | b'7' => {
            let _ = read_login_menu_byte(reader)?;
            Ok(LoginMenuKey::Home)
        }
        b'4' | b'8' => {
            let _ = read_login_menu_byte(reader)?;
            Ok(LoginMenuKey::End)
        }
        _ => Ok(LoginMenuKey::Ignore),
    }
}

#[cfg(test)]
fn read_login_menu_byte<R: Read>(reader: &mut R) -> io::Result<Option<u8>> {
    let mut byte = [0u8; 1];
    match reader.read(&mut byte) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(byte[0])),
        Err(err) if err.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod login_menu_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn login_menu_entries_explain_runtime_only_api_key_providers() {
        let entries = login_menu_entries();
        assert!(entries.len() > 5);

        let deepseek = entries
            .iter()
            .find(|entry| entry.title == "DeepSeek API key")
            .expect("deepseek guidance should be listed");
        assert_eq!(
            deepseek.action,
            LoginMenuAction::Guidance(LoginGuidanceKind::DeepSeekApiKey)
        );
        assert_eq!(deepseek.auth, "Runtime API key only");
        assert!(deepseek.usage.contains("no OAuth login"));

        let gemini_api_key = entries
            .iter()
            .find(|entry| entry.title == "Google Gemini API key")
            .expect("gemini API key guidance should be listed");
        assert_eq!(
            gemini_api_key.action,
            LoginMenuAction::Guidance(LoginGuidanceKind::GeminiApiKey)
        );
        assert!(gemini_api_key.command.contains("GEMINI_API_KEY"));

        let google_oauth = entries
            .iter()
            .find(|entry| entry.title == "Google Gemini OAuth")
            .expect("gemini OAuth login should be listed");
        assert_eq!(
            google_oauth.action,
            LoginMenuAction::Method(LoginMethod::Google)
        );
        assert!(google_oauth.auth.contains("OAuth"));
    }

    #[test]
    fn login_menu_layout_and_render_fit_short_terminals() {
        let entries = login_menu_entries();
        let layout = login_menu_layout_for_rows(8, entries.len());
        assert!(layout.compact);
        assert_eq!(layout.visible_items, 3);

        let selected = entries
            .iter()
            .position(|entry| entry.title == "DeepSeek API key")
            .expect("deepseek guidance should be listed");
        let offset = login_menu_window_offset(selected, 0, layout.visible_items, entries.len());
        let lines = render_login_menu(entries, selected, offset, layout, 50);

        assert_eq!(lines.len(), 8);
        assert!(lines.iter().any(|line| line.contains("DeepSeek API key")));
        assert!(lines.iter().any(|line| line.contains("Use/Cmd:")));
        assert!(lines.iter().all(|line| line.chars().count() <= 50));
    }

    #[test]
    fn login_menu_window_keeps_selected_item_visible() {
        assert_eq!(login_menu_window_offset(0, 0, 3, 9), 0);
        assert_eq!(login_menu_window_offset(2, 0, 3, 9), 0);
        assert_eq!(login_menu_window_offset(3, 0, 3, 9), 1);
        assert_eq!(login_menu_window_offset(8, 1, 3, 9), 6);
        assert_eq!(login_menu_window_offset(2, 6, 3, 9), 2);
        assert_eq!(login_menu_window_offset(8, 0, 20, 9), 0);
    }

    #[test]
    fn login_menu_reads_common_arrow_keys() {
        let mut up = Cursor::new(b"\x1b[A".to_vec());
        assert_eq!(read_login_menu_key(&mut up).unwrap(), LoginMenuKey::Up);

        let mut down = Cursor::new(b"\x1b[B".to_vec());
        assert_eq!(read_login_menu_key(&mut down).unwrap(), LoginMenuKey::Down);

        let mut page_down = Cursor::new(b"\x1b[6~".to_vec());
        assert_eq!(
            read_login_menu_key(&mut page_down).unwrap(),
            LoginMenuKey::PageDown
        );

        let mut digit = Cursor::new(b"8".to_vec());
        assert_eq!(
            read_login_menu_key(&mut digit).unwrap(),
            LoginMenuKey::Digit(8)
        );
    }

    #[test]
    fn login_menu_maps_crossterm_keys() {
        assert_eq!(
            login_menu_key_from_event(KeyEvent::new(
                KeyCode::Down,
                crossterm::event::KeyModifiers::NONE
            )),
            LoginMenuKey::Down
        );
        assert_eq!(
            login_menu_key_from_event(KeyEvent::new(
                KeyCode::Char('s'),
                crossterm::event::KeyModifiers::NONE
            )),
            LoginMenuKey::Ignore
        );
        assert_eq!(
            login_menu_key_from_event(KeyEvent::new(
                KeyCode::Char('7'),
                crossterm::event::KeyModifiers::NONE
            )),
            LoginMenuKey::Digit(7)
        );
        assert_eq!(
            login_menu_key_from_event(KeyEvent::new(
                KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE
            )),
            LoginMenuKey::Cancel
        );
    }
}
