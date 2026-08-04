use anyhow::{Context, Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::env;
use std::io::{self, IsTerminal};
use std::path::Path;
use terminal_ui::{
    tui_border_style, tui_connected_footer_block, tui_connected_header_block, tui_hint_style,
    tui_secondary_style, tui_title_style,
};

use super::import_export::{
    ProfileAuthUpdate, acquire_profile_lifecycle_lock, cleanup_profile_lifecycle_and_auth_journal,
    load_profile_state_with_profile_recovery_locked, prepare_existing_profile_lifecycle,
};
use crate::{
    AddProfileArgs, AppPaths, AppState, AppStateIoExt, ProfileProviderExt, ProfileSelector,
    absolutize, activate_profile, audit_log_event, collect_profile_summaries, default_codex_home,
    fetch_profile_identity, find_profile_by_identity, print_panel, read_auth_json_text,
    repair_missing_active_profile_and_save, resolve_profile_name, update_existing_profile_auth,
};

mod add_profile;
use add_profile::add_new_profile_to_state;

#[derive(Debug, Clone)]
struct ProfilePanel {
    title: String,
    fields: Vec<(String, String)>,
}

pub(crate) fn handle_add_profile(args: AddProfileArgs) -> Result<()> {
    prodex_profile_identity::validate_profile_name(&args.name)?;
    let source_kind = prodex_profile_identity::resolve_add_profile_source_kind(
        args.codex_home.is_some(),
        args.copy_from.is_some(),
        args.copy_current,
    )?;

    let paths = AppPaths::discover()?;
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, _) = load_profile_state_with_profile_recovery_locked(&paths, true)?;

    if state.profiles.contains_key(&args.name) {
        bail!("profile '{}' already exists", args.name);
    }

    let managed = source_kind.managed();
    let source_home = match source_kind {
        prodex_profile_identity::AddProfileSourceKind::CopyCurrent => {
            Some(default_codex_home(&paths)?)
        }
        prodex_profile_identity::AddProfileSourceKind::CopyFrom => {
            let copy_from = args.copy_from.as_ref().ok_or_else(|| {
                anyhow::anyhow!("internal error: copy-from path missing after validation")
            })?;
            Some(absolutize(copy_from.clone())?)
        }
        prodex_profile_identity::AddProfileSourceKind::ExternalHome
        | prodex_profile_identity::AddProfileSourceKind::EmptyManaged => None,
    };
    let activate_profile = prodex_profile_identity::should_activate_profile(
        state.active_profile.is_some(),
        args.activate,
    );
    let source_identity = read_source_profile_identity(source_home.as_deref())?;
    let source_email = source_identity
        .as_ref()
        .and_then(|identity| identity.email.clone());

    if update_duplicate_profile_if_needed(
        &paths,
        &mut state,
        &args.name,
        source_home.as_deref(),
        source_identity.as_ref(),
        activate_profile,
    )? {
        return Ok(());
    }

    let (codex_home, lifecycle_path) = add_new_profile_to_state(
        &paths,
        &mut state,
        &args,
        source_home.as_deref(),
        source_email,
        managed,
        activate_profile,
    )?;

    state.save(&paths)?;
    audit_log_event(
        "profile",
        "add",
        "success",
        serde_json::json!({
            "profile_name": args.name.clone(),
            "managed": managed,
            "activated": state.active_profile.as_deref() == Some(args.name.as_str()),
            "copied_source": source_home.is_some(),
            "codex_home": codex_home.display().to_string(),
            "source_home": source_home.as_ref().map(|path| path.display().to_string()),
        }),
    )?;

    let storage_message = if source_home.is_some() {
        "Source copied into managed profile home.".to_string()
    } else if managed {
        "Managed profile home created.".to_string()
    } else {
        "Existing CODEX_HOME registered.".to_string()
    };

    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Added profile '{}'.", args.name),
        ),
        ("Profile".to_string(), args.name.clone()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
        ("Storage".to_string(), storage_message),
    ];
    if state.active_profile.as_deref() == Some(args.name.as_str()) {
        fields.push(("Active".to_string(), args.name.clone()));
    }
    print_profile_panel("Profile Added", &fields)?;
    prodex_profile_export::cleanup_profile_lifecycle_journal(&lifecycle_path);

    Ok(())
}

fn read_source_profile_identity(
    source_home: Option<&Path>,
) -> Result<Option<prodex_profile_identity::ProfileIdentity>> {
    source_home.map(fetch_profile_identity).transpose()
}

fn update_duplicate_profile_if_needed(
    paths: &AppPaths,
    state: &mut AppState,
    requested_name: &str,
    source_home: Option<&Path>,
    source_identity: Option<&prodex_profile_identity::ProfileIdentity>,
    activate_profile: bool,
) -> Result<bool> {
    let Some(source) = source_home else {
        return Ok(false);
    };
    let Some(identity) = source_identity else {
        return Ok(false);
    };
    let Some(email) = identity.email.as_deref() else {
        return Ok(false);
    };
    let Some(profile_name) = find_profile_by_identity(state, identity)? else {
        return Ok(false);
    };
    let Ok(Some(auth_json)) = read_auth_json_text(source) else {
        return Ok(false);
    };

    let mut desired_profile = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?
        .clone();
    desired_profile.email = Some(email.to_string());
    let (lifecycle_path, auth_journal_path) = prepare_existing_profile_lifecycle(
        paths,
        "manage",
        state,
        &profile_name,
        &desired_profile,
        if activate_profile {
            Some(profile_name.clone())
        } else {
            state.active_profile.clone()
        },
        ProfileAuthUpdate {
            next_auth_json: Some(auth_json.clone()),
            next_provider_json: Some(serde_json::to_string(&desired_profile.provider)?),
            next_secret_files: Vec::new(),
            previous_secret_file_paths: &[],
            temporary_home: None,
        },
    )?;
    let updated = update_existing_profile_auth(
        paths,
        state,
        &profile_name,
        Some(email),
        &auth_json,
        activate_profile,
    )?;
    let updated_profile_name = updated.profile_name.clone();
    let updated_codex_home = updated.codex_home.clone();
    state.save(paths)?;
    audit_log_event(
        "profile",
        "add",
        "success",
        serde_json::json!({
            "profile_name": updated_profile_name.clone(),
            "requested_name": requested_name,
            "duplicate_email": true,
            "email": email,
            "updated_token_only": true,
            "source_home": source.display().to_string(),
            "codex_home": updated_codex_home.display().to_string(),
            "activated": state.active_profile.as_deref() == Some(updated_profile_name.as_str()),
        }),
    )?;

    let mut fields = vec![
        (
            "Result".to_string(),
            format!(
                "Detected duplicate account {email}. Updated auth token for profile '{}'.",
                updated_profile_name
            ),
        ),
        ("Account".to_string(), email.to_string()),
        ("Profile".to_string(), updated.profile_name.clone()),
        (
            "CODEX_HOME".to_string(),
            updated_codex_home.display().to_string(),
        ),
        (
            "Storage".to_string(),
            "Existing profile token updated.".to_string(),
        ),
    ];
    if state.active_profile.as_deref() == Some(updated.profile_name.as_str()) {
        fields.push(("Active".to_string(), updated.profile_name));
    }
    print_profile_panel("Profile Updated", &fields)?;
    cleanup_profile_lifecycle_and_auth_journal(&lifecycle_path, &auth_journal_path)?;
    Ok(true)
}

pub(crate) fn handle_list_profiles() -> Result<()> {
    let paths = AppPaths::discover()?;
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, _) = load_profile_state_with_profile_recovery_locked(&paths, true)?;
    repair_missing_active_profile_and_save(&paths, &mut state)?;

    if state.profiles.is_empty() {
        let fields = vec![
            ("Status".to_string(), "No profiles configured.".to_string()),
            (
                "Create".to_string(),
                "prodex profile add <name>".to_string(),
            ),
            (
                "Import".to_string(),
                "prodex profile import-current".to_string(),
            ),
            (
                "Import Copilot".to_string(),
                "prodex profile import copilot".to_string(),
            ),
        ];
        print_profile_panel("Profiles", &fields)?;
        return Ok(());
    }

    let summary_fields = vec![
        ("Count".to_string(), state.profiles.len().to_string()),
        (
            "Active".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    let mut panels = vec![ProfilePanel {
        title: "Profiles".to_string(),
        fields: summary_fields,
    }];

    for summary in collect_profile_summaries(&state) {
        let kind = if summary.managed {
            "managed"
        } else {
            "external"
        };

        let fields = vec![
            (
                "Current".to_string(),
                if summary.active {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
            ("Kind".to_string(), kind.to_string()),
            (
                "Provider".to_string(),
                summary.provider.display_name().to_string(),
            ),
            ("Auth".to_string(), summary.auth.label),
            (
                "Identity".to_string(),
                summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            ("Path".to_string(), summary.codex_home.display().to_string()),
        ];
        panels.push(ProfilePanel {
            title: format!("Profile {}", summary.name),
            fields,
        });
    }

    print_profile_panels(&panels)?;
    Ok(())
}

pub(crate) fn handle_set_active_profile(selector: ProfileSelector) -> Result<()> {
    let paths = AppPaths::discover()?;
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, _) = load_profile_state_with_profile_recovery_locked(&paths, true)?;
    let name = resolve_profile_name(&state, selector.profile.as_deref())?;
    activate_profile(&mut state, &name);
    state.save(&paths)?;

    let profile = state
        .profiles
        .get(&name)
        .with_context(|| format!("profile '{}' disappeared from state", name))?;
    audit_log_event(
        "profile",
        "set_active",
        "success",
        serde_json::json!({
            "profile_name": name.clone(),
            "codex_home": profile.codex_home.display().to_string(),
        }),
    )?;

    let fields = vec![
        ("Result".to_string(), format!("Active profile: {name}")),
        (
            "CODEX_HOME".to_string(),
            profile.codex_home.display().to_string(),
        ),
    ];
    print_profile_panel("Active Profile", &fields)?;
    Ok(())
}

pub(crate) fn handle_current_profile() -> Result<()> {
    let paths = AppPaths::discover()?;
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, _) = load_profile_state_with_profile_recovery_locked(&paths, true)?;
    repair_missing_active_profile_and_save(&paths, &mut state)?;

    let Some(active) = state.active_profile.as_deref() else {
        let mut fields = vec![("Status".to_string(), "No active profile.".to_string())];
        if state.profiles.len() == 1
            && let Some((name, profile)) = state.profiles.iter().next()
        {
            fields.push(("Only profile".to_string(), name.clone()));
            fields.push((
                "CODEX_HOME".to_string(),
                profile.codex_home.display().to_string(),
            ));
        }
        print_profile_panel("Active Profile", &fields)?;
        return Ok(());
    };

    let profile = state
        .profiles
        .get(active)
        .with_context(|| format!("active profile '{}' is missing", active))?;

    let fields = vec![
        ("Profile".to_string(), active.to_string()),
        (
            "CODEX_HOME".to_string(),
            profile.codex_home.display().to_string(),
        ),
        (
            "Managed".to_string(),
            if profile.managed {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
        (
            "Provider".to_string(),
            profile.provider.display_name().to_string(),
        ),
        (
            "Identity".to_string(),
            profile.email.as_deref().unwrap_or("-").to_string(),
        ),
        (
            "Auth".to_string(),
            profile.provider.auth_summary(&profile.codex_home).label,
        ),
    ];
    print_profile_panel("Active Profile", &fields)?;
    Ok(())
}

pub(super) fn print_profile_panel(title: &str, fields: &[(String, String)]) -> Result<()> {
    print_profile_panels(&[ProfilePanel {
        title: title.to_string(),
        fields: fields.to_vec(),
    }])
}

fn print_profile_panels(panels: &[ProfilePanel]) -> Result<()> {
    if profile_tui_should_scroll(panels)
        && let Ok(()) = print_profile_panels_scrollable(panels)
    {
        return Ok(());
    }

    let height = profile_tui_height(panels);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        for panel in panels {
            print_panel(&panel.title, &panel.fields)?;
        }
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Prodex Profiles", tui_title_style()),
            Span::raw("  "),
            Span::styled(format!("{} panel(s)", panels.len()), tui_secondary_style()),
        ]))
        .block(tui_connected_header_block(tui_border_style()));
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(profile_tui_text(panels))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

type ProfilePanelsTui = terminal_ui::AlternateScreenTerminal<io::Stdout>;

fn print_profile_panels_scrollable(panels: &[ProfilePanel]) -> Result<()> {
    let mut tui = ProfilePanelsTui::stdout("profile list")?;
    let mut scroll_offset = 0usize;
    loop {
        let total_lines = profile_tui_lines(panels).len();
        let size = tui.terminal.size()?;
        let body_height = profile_scroll_body_height(size.height);
        let max_scroll = profile_scroll_max_offset(total_lines, body_height);
        scroll_offset = scroll_offset.min(max_scroll);
        tui.terminal
            .draw(|frame| render_profile_panels_scroll_tui(frame, panels, scroll_offset))
            .context("failed to draw profile list TUI")?;

        if let Event::Key(key) = event::read().context("failed to read profile list input")?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => return Ok(()),
                KeyCode::Char('c') | KeyCode::Char('z')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    return Ok(());
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    scroll_offset = scroll_offset.saturating_add(1).min(max_scroll);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    scroll_offset = scroll_offset.saturating_sub(1);
                }
                KeyCode::PageDown => {
                    scroll_offset = scroll_offset.saturating_add(body_height).min(max_scroll);
                }
                KeyCode::PageUp => {
                    scroll_offset = scroll_offset.saturating_sub(body_height);
                }
                KeyCode::Home => scroll_offset = 0,
                KeyCode::End => scroll_offset = max_scroll,
                _ => {}
            }
        }
    }
}

fn render_profile_panels_scroll_tui(
    frame: &mut ratatui::Frame<'_>,
    panels: &[ProfilePanel],
    scroll_offset: usize,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let header = Paragraph::new(Line::from(vec![
        Span::styled("Prodex Profiles", tui_title_style()),
        Span::raw("  "),
        Span::styled(format!("{} panel(s)", panels.len()), tui_secondary_style()),
    ]))
    .block(tui_connected_header_block(tui_border_style()));
    frame.render_widget(header, chunks[0]);

    let total_lines = profile_tui_lines(panels);
    let body_height = usize::from(chunks[1].height).max(1);
    let body = Paragraph::new(Text::from(
        total_lines
            .iter()
            .skip(scroll_offset)
            .take(body_height)
            .cloned()
            .collect::<Vec<_>>(),
    ))
    .block(
        Block::default()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_style(tui_border_style()),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[1]);

    let max_scroll = profile_scroll_max_offset(total_lines.len(), body_height);
    let footer = Paragraph::new(Line::styled(
        profile_scroll_footer(scroll_offset, max_scroll),
        tui_hint_style().add_modifier(Modifier::BOLD),
    ))
    .block(tui_connected_footer_block(tui_border_style()));
    frame.render_widget(footer, chunks[2]);
}

fn profile_tui_should_scroll(panels: &[ProfilePanel]) -> bool {
    if !profile_scroll_tui_allowed() {
        return false;
    }
    profile_tui_lines(panels).len().saturating_add(6) > terminal_height()
}

fn profile_scroll_tui_allowed() -> bool {
    io::stdout().is_terminal()
        && env::var_os("CODEX_CI").is_none()
        && env::var("CI")
            .map(|value| {
                !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            })
            .unwrap_or(true)
}

fn terminal_height() -> usize {
    terminal::size()
        .map(|(_, height)| usize::from(height))
        .unwrap_or(24)
}

fn profile_scroll_body_height(terminal_height: u16) -> usize {
    usize::from(terminal_height).saturating_sub(6).max(1)
}

fn profile_scroll_max_offset(total_lines: usize, body_height: usize) -> usize {
    total_lines.saturating_sub(body_height.max(1))
}

fn profile_scroll_footer(scroll_offset: usize, max_scroll: usize) -> String {
    if max_scroll == 0 {
        "q close".to_string()
    } else {
        format!(
            "j/k scroll | pgup/pgdn page | home/end | q close | line {}/{}",
            scroll_offset.saturating_add(1),
            max_scroll.saturating_add(1)
        )
    }
}

fn profile_tui_height(panels: &[ProfilePanel]) -> u16 {
    let rows = profile_tui_lines(panels).len().saturating_add(4).max(4);
    let terminal_height = terminal::size()
        .map(|(_, height)| usize::from(height))
        .unwrap_or(24);
    rows.min(terminal_height).max(1) as u16
}

fn profile_tui_text(panels: &[ProfilePanel]) -> Text<'static> {
    Text::from(profile_tui_lines(panels))
}

fn profile_tui_lines(panels: &[ProfilePanel]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for panel in panels {
        lines.push(Line::styled(panel.title.clone(), tui_title_style()));
        let label_width = panel
            .fields
            .iter()
            .map(|(label, _)| terminal_ui::text_width(label))
            .max()
            .unwrap_or(0)
            .min(22);
        for (label, value) in &panel.fields {
            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "{label}{} ",
                        " ".repeat(label_width.saturating_sub(terminal_ui::text_width(label)))
                    ),
                    tui_secondary_style().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    value.clone(),
                    Style::default().fg(profile_value_color(label, value)),
                ),
            ]));
        }
    }
    lines
}

fn profile_value_color(label: &str, value: &str) -> Color {
    let lower = value.to_ascii_lowercase();
    if lower.contains("no active") || lower.contains("missing") || lower.contains("error") {
        Color::Red
    } else if lower.contains("active") || lower == "yes" || label == "Active" {
        Color::Green
    } else if label == "Provider"
        || label == "Auth"
        || label == "Runtime route"
        || label == "Identity"
    {
        Color::Cyan
    } else {
        Color::Reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn profile_tui_text_contains_panel_fields() {
        let panels = vec![ProfilePanel {
            title: "Profiles".to_string(),
            fields: vec![
                ("Active".to_string(), "main".to_string()),
                ("Provider".to_string(), "OpenAI".to_string()),
            ],
        }];
        let text = format!("{:?}", profile_tui_text(&panels));
        assert!(text.contains("Profiles"));
        assert!(text.contains("main"));
        assert!(text.contains("OpenAI"));
    }

    #[test]
    fn profile_tui_lines_do_not_pad_between_panels() {
        let panels = vec![
            ProfilePanel {
                title: "One".to_string(),
                fields: vec![("Active".to_string(), "main".to_string())],
            },
            ProfilePanel {
                title: "Two".to_string(),
                fields: vec![("Provider".to_string(), "OpenAI".to_string())],
            },
        ];

        let lines = profile_tui_lines(&panels);
        assert_eq!(lines.len(), 4);
        assert!(!format!("{:?}", lines[2]).contains("\"\""));
        assert!(format!("{:?}", lines[2]).contains("Two"));
    }

    #[test]
    fn profile_value_color_highlights_status() {
        assert_eq!(profile_value_color("Active", "main"), Color::Green);
        assert_eq!(
            profile_value_color("Status", "No active profile."),
            Color::Red
        );
        assert_eq!(profile_value_color("Provider", "OpenAI"), Color::Cyan);
    }

    #[test]
    fn profile_scroll_bounds_allow_full_overflow_range() {
        assert_eq!(profile_scroll_body_height(10), 4);
        assert_eq!(profile_scroll_max_offset(20, 4), 16);
        assert_eq!(profile_scroll_max_offset(4, 4), 0);
        assert_eq!(profile_scroll_max_offset(3, 4), 0);
    }

    #[test]
    fn profile_scroll_footer_reports_current_line() {
        assert_eq!(profile_scroll_footer(0, 0), "q close");
        let footer = profile_scroll_footer(2, 8);
        assert!(footer.contains("j/k scroll"));
        assert!(footer.contains("line 3/9"));
    }

    #[test]
    fn copy_source_identity_read_errors_are_propagated() {
        let root = std::env::temp_dir().join(format!(
            "prodex-manage-copy-source-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("config.toml"),
            "model_provider = 'amazon-bedrock'\n",
        )
        .unwrap();
        fs::write(secret_store::auth_json_path(&root), "{").unwrap();

        let error = read_source_profile_identity(Some(&root))
            .expect_err("copying a source with unreadable identity must fail");

        assert!(format!("{error:#}").contains("failed to read account identity"));
        let _ = fs::remove_dir_all(root);
    }
}
