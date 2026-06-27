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
use std::io::{self, IsTerminal, Write};
use terminal_ui::{
    tui_border_style, tui_hint_style, tui_primary_style, tui_secondary_style, tui_success_style,
    tui_title_style,
};

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
        Span::styled("Prodex Login", tui_title_style()),
        Span::raw("  "),
        Span::styled(
            format!(
                "methods {}-{} of {}",
                offset.saturating_add(1).min(entries.len()),
                end,
                entries.len()
            ),
            tui_secondary_style(),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(tui_border_style()),
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
                    tui_hint_style().add_modifier(Modifier::BOLD)
                } else {
                    tui_primary_style()
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
            .border_style(tui_border_style()),
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
                .border_style(tui_border_style()),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(detail, chunks[2]);
}

fn login_menu_detail_text(entry: &LoginMenuEntry, layout: LoginMenuLayout) -> Text<'static> {
    if layout.compact {
        return Text::from(vec![
            Line::from(vec![
                Span::styled(
                    "Select login method - Provide your own API key  ",
                    tui_secondary_style(),
                ),
                Span::styled("Provider ", tui_secondary_style()),
                Span::styled(entry.provider, tui_success_style()),
                Span::raw(" | "),
                Span::styled(entry.auth, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("Use ", tui_secondary_style()),
                Span::raw(entry.usage),
            ]),
            Line::from(vec![
                Span::styled("Cmd ", tui_secondary_style()),
                Span::styled(entry.command, tui_hint_style()),
            ]),
        ]);
    }

    Text::from(vec![
        Line::from(vec![
            Span::styled(
                "Select login method - Provide your own API key  ",
                tui_secondary_style(),
            ),
            Span::styled("Provider ", tui_secondary_style()),
            Span::styled(entry.provider, tui_success_style()),
        ]),
        Line::from(vec![
            Span::styled("Auth     ", tui_secondary_style()),
            Span::styled(entry.auth, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Use      ", tui_secondary_style()),
            Span::raw(entry.usage),
        ]),
        Line::from(vec![
            Span::styled("Command  ", tui_secondary_style()),
            Span::styled(entry.command, tui_hint_style()),
        ]),
        Line::from(Span::styled(
            "Select login method | Up/Down move | PageUp/PageDown scroll | Enter select | 1-9 quick select | q cancel",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        )),
    ])
}

#[cfg(test)]
#[path = "../../../tests/src/profile_commands/login_menu.rs"]
mod login_menu_tests;
