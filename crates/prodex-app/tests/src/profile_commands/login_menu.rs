use super::*;
use std::io::{Cursor, Read};

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

fn read_login_menu_byte<R: Read>(reader: &mut R) -> io::Result<Option<u8>> {
    let mut byte = [0u8; 1];
    match reader.read(&mut byte) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(byte[0])),
        Err(err) if err.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(err) => Err(err),
    }
}

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
