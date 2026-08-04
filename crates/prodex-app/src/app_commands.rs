use super::*;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::IsTerminal;
use terminal_ui::{
    fit_cell, tui_border_style, tui_connected_footer_block, tui_connected_header_block,
    tui_detail_style, tui_hint_style, tui_primary_style, tui_secondary_style, tui_success_style,
    tui_title_style,
};

const SUPER_PROMPT_MAX_TEXT_CHARS: usize = 256;
const SUPER_PROMPT_MAX_MENU_CANDIDATES: usize = 32;

mod app_server_broker;
mod audit;
mod broker;
mod capability;
mod child_process;
mod cleanup;
mod context;
mod dashboard;
mod doctor;
mod gateway;
mod gui;
mod info;
mod info_handler;
mod log;
mod log_format;
mod log_tui;
mod log_upstream;
mod log_upstream_payload;
mod mcp_jsonl_bridge;
mod ping;
mod presidio;
mod quota;
mod redeem;
pub(crate) mod runtime_launch;
mod selection;
mod session;
mod shared;
mod status;

pub(crate) use self::app_server_broker::*;
pub(crate) use self::audit::*;
pub(crate) use self::broker::*;
pub(crate) use self::capability::*;
pub(crate) use self::child_process::*;
pub(crate) use self::cleanup::*;
pub(crate) use self::context::*;
pub(crate) use self::dashboard::*;
pub(crate) use self::doctor::*;
pub(crate) use self::gateway::*;
pub(crate) use self::gui::*;
pub(crate) use self::info::*;
pub(crate) use self::info_handler::*;
pub(crate) use self::log::*;
pub(crate) use self::mcp_jsonl_bridge::*;
pub(crate) use self::ping::*;
pub(crate) use self::presidio::*;
pub(crate) use self::quota::*;
pub(crate) use self::redeem::*;
pub(crate) use self::selection::*;
pub(crate) use self::session::*;
pub(crate) use self::shared::*;
pub(crate) use self::status::*;

pub(super) fn handle_super(args: SuperArgs) -> Result<()> {
    args.validate_urls().map_err(anyhow::Error::msg)?;
    reject_sub_agent_recursion_reenable(&args)?;
    let interactive = super_prompt_is_interactive();
    let (use_presidio, sub_agent) = resolve_super_launch_decisions_with_prompts(
        &args,
        interactive,
        prompt_super_presidio_opt_in,
        || prompt_super_sub_agent_configuration(&args),
    )?;
    if matches!(
        args.cli,
        Some(
            SuperCliAgent::Gemini
                | SuperCliAgent::Copilot
                | SuperCliAgent::Kiro
                | SuperCliAgent::Agy
        )
    ) {
        return crate::runtime_gemini_cli::handle_super_native_cli(args, use_presidio, sub_agent);
    }
    handle_super_runtime_tools(
        args.into_runtime_tool_args_with_presidio(use_presidio),
        sub_agent,
    )
}

pub(super) fn resolve_super_sub_agent(
    args: &SuperArgs,
    interactive: bool,
) -> Result<Option<ResolvedSuperSubAgent>> {
    resolve_super_sub_agent_with_prompt(args, interactive && super_prompt_is_interactive(), || {
        prompt_super_sub_agent_configuration(args)
    })
}

fn resolve_super_launch_decisions_with_prompts(
    args: &SuperArgs,
    interactive: bool,
    prompt_presidio: impl FnOnce() -> Result<bool>,
    prompt_sub_agent: impl FnOnce() -> Result<Option<SubAgentConfig>>,
) -> Result<(bool, Option<ResolvedSuperSubAgent>)> {
    let use_presidio = if matches!(args.cli, Some(SuperCliAgent::Kiro | SuperCliAgent::Agy)) {
        false
    } else {
        match args.presidio_preference() {
            Some(use_presidio) => use_presidio,
            None if interactive => prompt_presidio()?,
            None => false,
        }
    };
    let mut sub_agent = resolve_super_sub_agent_with_prompt(args, interactive, prompt_sub_agent)?;
    if let Some(sub_agent) = sub_agent.as_mut() {
        sub_agent.presidio_enabled = use_presidio;
    }
    Ok((use_presidio, sub_agent))
}

fn resolve_super_sub_agent_with_prompt(
    args: &SuperArgs,
    interactive: bool,
    prompt: impl FnOnce() -> Result<Option<SubAgentConfig>>,
) -> Result<Option<ResolvedSuperSubAgent>> {
    let preference = args.sub_agent_preference();
    let explicitly_enabled = matches!(&preference, SubAgentPreference::Enabled(_));
    if matches!(
        sub_agent_recursion_policy(),
        SubAgentRecursionPolicy::Disabled
    ) {
        if explicitly_enabled {
            bail!("--sub-agent cannot be re-enabled while {SUB_AGENT_RECURSION_MARKER} is set");
        }
        return Ok(None);
    }

    if matches!(
        args.cli,
        Some(
            SuperCliAgent::Gemini
                | SuperCliAgent::Copilot
                | SuperCliAgent::Kiro
                | SuperCliAgent::Agy
        )
    ) {
        if matches!(&preference, SubAgentPreference::Enabled(_)) {
            bail!(
                "--sub-agent is supported only on the Codex Super bridge, not native CLI launches"
            );
        }
        return Ok(None);
    }

    let config = match preference {
        SubAgentPreference::Disabled => return Ok(None),
        SubAgentPreference::Enabled(config) => config,
        SubAgentPreference::Unspecified if interactive => match prompt()? {
            Some(config) => config,
            None => return Ok(None),
        },
        SubAgentPreference::Unspecified => return Ok(None),
    };

    debug_assert!(explicitly_enabled || interactive);
    resolve_super_sub_agent_config(config, resolve_super_launch_target(&args.codex_args)).map(Some)
}

fn prompt_super_sub_agent_configuration(args: &SuperArgs) -> Result<Option<SubAgentConfig>> {
    if !prompt_super_sub_agent_opt_in()? {
        return Ok(None);
    }
    prompt_super_sub_agent_config(
        SubAgentConfig::default(),
        false,
        false,
        false,
        args.url.as_deref(),
    )
    .map(Some)
}

fn reject_sub_agent_recursion_reenable(args: &SuperArgs) -> Result<()> {
    if matches!(
        sub_agent_recursion_policy(),
        SubAgentRecursionPolicy::Disabled
    ) && matches!(args.sub_agent_preference(), SubAgentPreference::Enabled(_))
    {
        bail!("--sub-agent cannot be re-enabled while {SUB_AGENT_RECURSION_MARKER} is set");
    }
    Ok(())
}

fn super_prompt_is_interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

fn prompt_super_sub_agent_opt_in() -> Result<bool> {
    Ok(prompt_super_choice(
        "Sub-agent opt-in",
        &["enable".to_string(), "skip".to_string()],
        1,
        true,
    )? == 0)
}

fn prompt_super_sub_agent_config(
    mut config: SubAgentConfig,
    provider_explicit: bool,
    model_explicit: bool,
    effort_explicit: bool,
    main_local_url: Option<&str>,
) -> Result<SubAgentConfig> {
    if !provider_explicit {
        let all_providers = canonical_sub_agent_providers();
        let providers = &all_providers[..all_providers.len().min(SUPER_PROMPT_MAX_MENU_CANDIDATES)];
        let choices = providers
            .iter()
            .map(|provider| provider_display_name(*provider).to_string())
            .collect::<Vec<_>>();
        let selected = providers
            .iter()
            .position(|provider| *provider == config.provider)
            .unwrap_or(0);
        config.provider =
            providers[prompt_super_choice("Sub-agent provider", &choices, selected, false)?];
    }

    if !model_explicit {
        let all_models = canonical_sub_agent_models(config.provider);
        let model_limit = SUPER_PROMPT_MAX_MENU_CANDIDATES.saturating_sub(2);
        let models = &all_models[..all_models.len().min(model_limit)];
        let mut choices = vec!["provider default".to_string()];
        choices.extend(
            models
                .iter()
                .map(|model| model.id.to_string())
                .collect::<Vec<_>>(),
        );
        choices.push("custom model…".to_string());
        let selected = config
            .model
            .as_deref()
            .and_then(|model| prodex_provider_core::provider_model_spec(config.provider, model))
            .and_then(|model| {
                models
                    .iter()
                    .position(|candidate| candidate.id == model.id)
                    .map(|index| index + 1)
            })
            .unwrap_or(0);
        let selected = prompt_super_choice("Sub-agent model", &choices, selected, false)?;
        if selected == 0 {
            config.model = None;
        } else if selected == models.len() + 1 {
            config.model = Some(prompt_super_text(
                "Custom sub-agent model",
                config.model.as_deref().unwrap_or_default(),
            )?);
        } else {
            config.model = Some(models[selected - 1].id.to_string());
        }
    }

    if !effort_explicit {
        let mut efforts = vec![("provider default".to_string(), None)];
        efforts.extend(
            canonical_sub_agent_efforts(config.provider, config.model.as_deref())
                .into_iter()
                .map(|effort| (effort.as_str().to_string(), Some(effort))),
        );
        let choices = efforts
            .iter()
            .map(|(label, _)| label.clone())
            .collect::<Vec<_>>();
        let selected = efforts
            .iter()
            .position(|(_, effort)| *effort == config.model_reasoning_effort)
            .unwrap_or(0);
        config.model_reasoning_effort = efforts
            [prompt_super_choice("Sub-agent reasoning effort", &choices, selected, false)?]
        .1;
    }

    if config.provider == prodex_provider_core::ProviderId::Local && config.url.is_none() {
        config.url = Some(prompt_super_text(
            "Local sub-agent URL",
            main_local_url.unwrap_or("http://127.0.0.1:11434/v1"),
        )?);
    }
    Ok(config)
}

fn prompt_super_choice(
    title: &str,
    choices: &[String],
    selected: usize,
    escape_selects_last: bool,
) -> Result<usize> {
    if choices.is_empty() {
        bail!("Sub-agent prompt has no choices");
    }
    let mut tui = terminal_ui::AlternateScreenTerminal::stderr("Super sub-agent prompt TUI")?;
    let mut selected = selected.min(choices.len().saturating_sub(1));
    loop {
        tui.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(3),
                ])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled("Prodex Super", tui_title_style()),
                Span::raw("  "),
                Span::styled(title, tui_detail_style()),
            ]))
            .block(tui_connected_header_block(tui_border_style()));
            frame.render_widget(header, chunks[0]);
            let visible = visible_choice_range(selected, choices.len(), chunks[1].height);
            let start = visible.start;
            let lines = choices[visible.start..visible.end]
                .iter()
                .enumerate()
                .map(|(offset, choice)| (start + offset, choice))
                .map(|(index, choice)| {
                    let choice = bounded_tui_text(choice, frame.area().width.saturating_sub(4));
                    if index == selected {
                        Line::from(vec![
                            Span::styled("› ", tui_success_style()),
                            Span::styled(choice, tui_primary_style().add_modifier(Modifier::BOLD)),
                        ])
                    } else {
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(choice, tui_secondary_style()),
                        ])
                    }
                })
                .collect::<Vec<_>>();
            let body = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::LEFT | Borders::RIGHT)
                        .border_style(tui_border_style()),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(body, chunks[1]);
            let footer = Paragraph::new(Line::from(vec![
                Span::styled("↑/↓", tui_hint_style()),
                Span::raw(" choose  "),
                Span::styled("enter", tui_success_style()),
                Span::raw(" select  "),
                Span::styled("esc", tui_hint_style()),
                Span::raw(if escape_selects_last {
                    " skip"
                } else {
                    " cancel"
                }),
            ]))
            .block(tui_connected_footer_block(tui_border_style()));
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.checked_sub(1).unwrap_or(choices.len() - 1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1) % choices.len();
                }
                KeyCode::Enter => return Ok(selected),
                KeyCode::Esc if escape_selects_last => return Ok(choices.len() - 1),
                KeyCode::Esc => bail!("Sub-agent prompt cancelled"),
                KeyCode::Char('c') | KeyCode::Char('z')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    bail!("Sub-agent prompt cancelled")
                }
                _ => {}
            }
        }
    }
}

fn visible_choice_range(selected: usize, len: usize, height: u16) -> std::ops::Range<usize> {
    let visible = usize::from(height).max(1).min(len);
    let start = selected
        .saturating_sub(visible / 2)
        .min(len.saturating_sub(visible));
    start..start + visible
}

fn bounded_tui_text(value: &str, width: u16) -> String {
    fit_cell(value, usize::from(width).max(1))
}

fn prompt_super_text(title: &str, initial: &str) -> Result<String> {
    let mut tui = terminal_ui::AlternateScreenTerminal::stderr("Super sub-agent text prompt TUI")?;
    let mut value = initial
        .chars()
        .take(SUPER_PROMPT_MAX_TEXT_CHARS)
        .collect::<String>();
    loop {
        tui.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(3),
                ])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled("Prodex Super", tui_title_style()),
                Span::raw("  "),
                Span::styled(title, tui_detail_style()),
            ]))
            .block(tui_connected_header_block(tui_border_style()));
            frame.render_widget(header, chunks[0]);
            let value_display = bounded_tui_text(&value, frame.area().width.saturating_sub(4));
            let body = Paragraph::new(Line::from(vec![
                Span::styled(value_display, tui_primary_style()),
                Span::styled("▌", tui_success_style()),
            ]))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: true });
            frame.render_widget(body, chunks[1]);
            let footer = Paragraph::new(Line::from(vec![
                Span::styled("enter", tui_success_style()),
                Span::raw(" accept  "),
                Span::styled("esc", tui_hint_style()),
                Span::raw(" cancel"),
            ]))
            .block(tui_connected_footer_block(tui_border_style()));
            frame.render_widget(footer, chunks[2]);
        })?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Enter if !value.trim().is_empty() => return Ok(value),
                KeyCode::Backspace => {
                    value.pop();
                }
                KeyCode::Char(character)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && value.chars().count() < SUPER_PROMPT_MAX_TEXT_CHARS =>
                {
                    value.push(character);
                }
                KeyCode::Esc => bail!("Sub-agent prompt cancelled"),
                KeyCode::Char('c') | KeyCode::Char('z')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    bail!("Sub-agent prompt cancelled")
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    runtime_launch::resolve_runtime_launch_profile_name(state, requested)
}

pub(super) fn prompt_super_presidio_opt_in() -> Result<bool> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Ok(false);
    }

    let mut tui = terminal_ui::AlternateScreenTerminal::stderr("Presidio prompt TUI")?;
    loop {
        tui.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(3),
                ])
                .split(frame.area());
            let header = Paragraph::new(Line::from(vec![
                Span::styled("Prodex Super", tui_title_style()),
                Span::raw("  "),
                Span::styled("Presidio opt-in", tui_detail_style()),
            ]))
            .block(tui_connected_header_block(tui_border_style()));
            frame.render_widget(header, chunks[0]);

            let body = Paragraph::new(vec![
                Line::from(Span::styled(
                    "Use Presidio for data safety?",
                    tui_primary_style().add_modifier(Modifier::BOLD),
                )),
                Line::raw(""),
                Line::from(Span::styled(
                    "Detected sensitive data is redacted from request bodies before upstream delivery.",
                    tui_secondary_style(),
                )),
            ])
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(body, chunks[1]);

            let footer = Paragraph::new(Line::from(vec![
                Span::styled("y", tui_success_style()),
                Span::raw(" enable  "),
                Span::styled("n", tui_hint_style()),
                Span::raw(" skip  "),
                Span::styled("enter", tui_hint_style()),
                Span::raw(" skip  "),
                Span::styled("esc", tui_hint_style()),
                Span::raw(" skip"),
            ]))
            .block(tui_connected_footer_block(tui_border_style()));
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Enter | KeyCode::Esc => {
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('z')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    bail!("Presidio prompt cancelled");
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod sub_agent_prompt_tests {
    use super::{
        bounded_tui_text, resolve_super_launch_decisions_with_prompts, resolve_super_sub_agent,
        visible_choice_range,
    };
    use prodex_cli::{SubAgentConfig, SubAgentReasoningEffort, SuperLaunchTarget};
    use prodex_provider_core::ProviderId;
    use std::cell::RefCell;

    const SESSION_ID: &str = "019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9";

    fn super_args(values: &[&str]) -> prodex_cli::SuperArgs {
        let mut argv = vec!["prodex", "s"];
        argv.extend(values.iter().copied());
        let crate::Commands::Super(mut args) =
            crate::parse_cli_command_from(argv).expect("Super command should parse")
        else {
            panic!("expected Super command");
        };
        args.extract_super_overrides_from_codex_args()
            .expect("Super tail should extract");
        args
    }

    #[test]
    fn choice_window_stays_bounded_and_keeps_selection_visible() {
        assert_eq!(visible_choice_range(0, 20, 4), 0..4);
        assert_eq!(visible_choice_range(19, 20, 4), 16..20);
        assert_eq!(visible_choice_range(10, 20, 4), 8..12);
        assert_eq!(visible_choice_range(10, 20, 1), 10..11);
    }

    #[test]
    fn choice_text_is_bounded_to_terminal_width() {
        assert_eq!(bounded_tui_text("abcdef", 1), ".");
        assert_eq!(bounded_tui_text("abcdef", 4), "a...");
        assert_eq!(bounded_tui_text("abc", 4), "abc");
        assert_eq!(bounded_tui_text("界界界", 5), "界...");
    }

    #[test]
    fn explicit_enable_skips_sub_agent_wizard() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(super::SUB_AGENT_RECURSION_MARKER);
        let command = crate::parse_cli_command_from(["prodex", "s", "--sub-agent"])
            .expect("explicit sub-agent command should parse");
        let crate::Commands::Super(args) = command else {
            panic!("expected Super command");
        };
        let resolved = resolve_super_sub_agent(&args, true)
            .expect("explicit enable should not open a wizard")
            .expect("explicit enable should resolve");
        assert_eq!(resolved.provider, prodex_provider_core::ProviderId::OpenAi);
        assert_eq!(resolved.model, None);
        assert_eq!(resolved.effort, None);
    }

    #[test]
    fn pure_interactive_resolution_prompts_presidio_before_resumed_sub_agent_config() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(super::SUB_AGENT_RECURSION_MARKER);
        let args = super_args(&[SESSION_ID]);
        let calls = RefCell::new(Vec::new());
        let (presidio, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &args,
            true,
            || {
                calls.borrow_mut().push("presidio");
                Ok(true)
            },
            || {
                calls.borrow_mut().push("sub-agent");
                Ok(Some(SubAgentConfig {
                    provider: ProviderId::Kiro,
                    model: Some("gpt-5.6-luna".to_string()),
                    model_reasoning_effort: Some(SubAgentReasoningEffort::Max),
                    url: None,
                }))
            },
        )
        .expect("interactive resolution should succeed");

        assert_eq!(&*calls.borrow(), &["presidio", "sub-agent"]);
        assert!(presidio);
        let sub_agent = sub_agent.expect("sub-agent should be enabled");
        assert!(sub_agent.presidio_enabled);
        assert_eq!(sub_agent.provider, ProviderId::Kiro);
        assert_eq!(sub_agent.model.as_deref(), Some("gpt-5.6-luna"));
        assert_eq!(sub_agent.effort, Some(SubAgentReasoningEffort::Max));
        assert_eq!(
            sub_agent.target,
            SuperLaunchTarget::Resume {
                session_id: SESSION_ID.to_string()
            }
        );
    }

    #[test]
    fn pure_interactive_resolution_can_disable_presidio_and_sub_agents() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(super::SUB_AGENT_RECURSION_MARKER);
        let calls = RefCell::new(Vec::new());
        let (presidio, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &super_args(&[SESSION_ID]),
            true,
            || {
                calls.borrow_mut().push("presidio");
                Ok(false)
            },
            || {
                calls.borrow_mut().push("sub-agent");
                Ok(None)
            },
        )
        .expect("interactive skip should succeed");

        assert_eq!(&*calls.borrow(), &["presidio", "sub-agent"]);
        assert!(!presidio);
        assert!(sub_agent.is_none());
    }

    #[test]
    fn pure_resolution_skips_prompts_for_non_tty_and_fully_configured_resume() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(super::SUB_AGENT_RECURSION_MARKER);
        let (presidio, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &super_args(&[]),
            false,
            || panic!("non-TTY Presidio prompt must not run"),
            || panic!("non-TTY sub-agent prompt must not run"),
        )
        .expect("non-TTY defaults should resolve");
        assert!(!presidio);
        assert!(sub_agent.is_none());

        let args = super_args(&[
            "--presidio",
            "--sub-agent",
            "--sub-agent-provider",
            "copilot",
            "--sub-agent-model",
            "configured-model",
            "--sub-agent-model-reasoning-effort",
            "xhigh",
            SESSION_ID,
        ]);
        let (presidio, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &args,
            true,
            || panic!("explicit Presidio must skip the prompt"),
            || panic!("explicit sub-agent config must skip the wizard"),
        )
        .expect("explicit resume should resolve");
        assert!(presidio);
        let sub_agent = sub_agent.expect("explicit sub-agent should resolve");
        assert_eq!(sub_agent.provider, ProviderId::Copilot);
        assert_eq!(sub_agent.model.as_deref(), Some("configured-model"));
        assert_eq!(sub_agent.effort, Some(SubAgentReasoningEffort::XHigh));
        assert!(sub_agent.presidio_enabled);
        assert_eq!(
            sub_agent.target,
            SuperLaunchTarget::Resume {
                session_id: SESSION_ID.to_string()
            }
        );
    }

    #[test]
    fn recursion_marker_rejects_explicit_reenable() {
        let _marker = crate::test_support::TestEnvVarGuard::set(
            super::SUB_AGENT_RECURSION_MARKER,
            "unexpected-value",
        );
        let command = crate::parse_cli_command_from(["prodex", "s", "--sub-agent"])
            .expect("explicit sub-agent command should parse");
        let crate::Commands::Super(args) = command else {
            panic!("expected Super command");
        };
        let error = resolve_super_sub_agent(&args, true)
            .expect_err("recursion marker must fail closed")
            .to_string();
        assert!(error.contains("PRODEX_SUB_AGENT"), "{error}");
    }
}
