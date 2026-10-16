use super::ResolvedMainAgentConfig;
use crate::{
    ResolvedSuperSubAgent, SUB_AGENT_RECURSION_MARKER, SubAgentRecursionPolicy,
    canonical_sub_agent_providers, effective_provider_model_catalog, provider_display_name,
    resolve_super_launch_target, resolve_super_sub_agent_config, sub_agent_recursion_policy,
};
use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use prodex_cli::{
    DEFAULT_SUB_AGENT_MAX_CONCURRENCY, HARD_MAX_SUB_AGENT_CONCURRENCY, SubAgentConcurrencySource,
    SubAgentConfig, SubAgentMaxConcurrency, SubAgentPreference, SuperArgs,
};
use prodex_mojo_core::launch::{
    SuperPromptChoiceKey, SuperPromptLabelKind, SuperPromptTextAction, SuperPromptTextKey,
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{self, IsTerminal};
use terminal_ui::{
    fit_cell, tui_border_style, tui_connected_footer_block, tui_connected_header_block,
    tui_detail_style, tui_hint_style, tui_primary_style, tui_secondary_style, tui_success_style,
    tui_title_style,
};

#[path = "super_prompt/choices.rs"]
mod choices;
pub(super) use choices::{
    prompt_super_choice, super_prompt_effort_choice_index, super_prompt_effort_label,
    super_prompt_model_choice_id_label, super_prompt_model_choice_index,
    super_prompt_model_choice_label, super_prompt_provider_choice_index,
    super_prompt_provider_choice_label,
};

pub(super) fn prompt_super_main_agent_configuration(
    args: &SuperArgs,
    locked_provider: Option<prodex_provider_core::ProviderId>,
) -> Result<ResolvedMainAgentConfig> {
    // Ordinary Super chooses agent/provider here; model and effort come from the
    // provider-scoped remembered preference resolver.
    prompt_super_main_agent_configuration_with_options(args, locked_provider, false)
}

pub(super) fn prompt_super_expose_main_agent_configuration(
    args: &SuperArgs,
    locked_provider: Option<prodex_provider_core::ProviderId>,
) -> Result<ResolvedMainAgentConfig> {
    // Expose is a long-lived launch boundary. Resolve and freeze the complete
    // main-agent selection once so every later prodex_super_start inherits it.
    prompt_super_main_agent_configuration_with_options(args, locked_provider, true)
}

fn prompt_super_main_agent_configuration_with_options(
    args: &SuperArgs,
    locked_provider: Option<prodex_provider_core::ProviderId>,
    prompt_model_and_effort: bool,
) -> Result<ResolvedMainAgentConfig> {
    let providers = locked_provider.map_or_else(
        || {
            prodex_provider_core::provider_implementation_registry()
                .iter()
                .map(|descriptor| descriptor.provider())
                .collect::<Vec<_>>()
        },
        |provider| vec![provider],
    );
    let choices = providers
        .iter()
        .map(|provider| super_prompt_provider_choice_label(*provider, locked_provider.is_some()))
        .collect::<Vec<_>>();
    let selected_provider = super_prompt_provider_choice_index(&providers, locked_provider);
    let provider =
        providers[prompt_super_choice("Main-agent provider", &choices, selected_provider, false)?];
    let local_url = if provider == prodex_provider_core::ProviderId::Local {
        if prompt_model_and_effort && args.url.is_some() {
            args.url.clone()
        } else {
            Some(prompt_super_text(
                "Main-agent local URL",
                args.url.as_deref().unwrap_or("http://127.0.0.1:11434/v1"),
            )?)
        }
    } else {
        None
    };
    let (model, reasoning_effort) = super::super_main_prompt::resolve_main_model_and_effort(
        args,
        provider,
        prompt_model_and_effort,
    )?;
    Ok(ResolvedMainAgentConfig {
        provider,
        model,
        reasoning_effort,
        local_url,
    })
}

pub(super) fn resolve_super_sub_agent_with_prompt(
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
    let mut sub_agent =
        resolve_super_sub_agent_config(config, resolve_super_launch_target(&args.codex_args))?;
    let required_tools = args
        .required_tools
        .iter()
        .copied()
        .collect::<prodex_optional_tools::OptionalToolSet>();
    sub_agent.required_tools = required_tools.iter().collect();
    Ok(Some(sub_agent))
}

pub(super) fn prompt_super_sub_agent_configuration(
    _args: &SuperArgs,
) -> Result<Option<SubAgentConfig>> {
    if !prompt_super_sub_agent_opt_in()? {
        return Ok(None);
    }
    prompt_super_sub_agent_config(SubAgentConfig::default(), false, false, false).map(Some)
}

pub(super) fn reject_sub_agent_recursion_reenable(args: &SuperArgs) -> Result<()> {
    if matches!(
        sub_agent_recursion_policy(),
        SubAgentRecursionPolicy::Disabled
    ) && matches!(args.sub_agent_preference(), SubAgentPreference::Enabled(_))
    {
        bail!("--sub-agent cannot be re-enabled while {SUB_AGENT_RECURSION_MARKER} is set");
    }
    Ok(())
}

pub(super) fn super_prompt_is_interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

fn prompt_super_sub_agent_opt_in() -> Result<bool> {
    Ok(prompt_super_choice(
        "Use sub-agents?",
        &["yes".to_string(), "no".to_string()],
        1,
        true,
    )? == 0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SuperSubAgentPromptStep {
    Provider,
    LocalUrl,
    Model,
    ReasoningEffort,
    MaxConcurrency,
}

pub(super) fn super_sub_agent_prompt_steps(
    config: &SubAgentConfig,
    provider_explicit: bool,
    model_explicit: bool,
    effort_explicit: bool,
) -> Vec<SuperSubAgentPromptStep> {
    let plan = prodex_mojo_core::sub_agent_policy::prompt_step_plan(
        provider_explicit,
        config.provider == prodex_provider_core::ProviderId::Local,
        config.url.is_some(),
        model_explicit,
        effort_explicit,
    )
    .expect("Mojo sub-agent prompt-step policy returned invalid output");
    [
        (plan.provider, SuperSubAgentPromptStep::Provider),
        (plan.local_url, SuperSubAgentPromptStep::LocalUrl),
        (plan.model, SuperSubAgentPromptStep::Model),
        (
            plan.reasoning_effort,
            SuperSubAgentPromptStep::ReasoningEffort,
        ),
        (
            plan.max_concurrency,
            SuperSubAgentPromptStep::MaxConcurrency,
        ),
    ]
    .into_iter()
    .filter_map(|(enabled, step)| enabled.then_some(step))
    .collect()
}

fn prompt_super_sub_agent_config(
    config: SubAgentConfig,
    provider_explicit: bool,
    model_explicit: bool,
    effort_explicit: bool,
) -> Result<SubAgentConfig> {
    run_super_sub_agent_prompt_steps(
        config,
        provider_explicit,
        model_explicit,
        effort_explicit,
        |step, config| {
            match step {
                SuperSubAgentPromptStep::Provider => {
                    let providers = canonical_sub_agent_providers();
                    let choices = providers
                        .iter()
                        .map(|provider| super_prompt_provider_choice_label(*provider, false))
                        .collect::<Vec<_>>();
                    let selected =
                        super_prompt_provider_choice_index(providers, Some(config.provider));
                    config.provider = providers
                        [prompt_super_choice("Sub-agent provider", &choices, selected, false)?];
                }
                SuperSubAgentPromptStep::LocalUrl => {
                    config.url = Some(prompt_super_text(
                        "Sub-agent local URL",
                        "http://127.0.0.1:11434/v1",
                    )?);
                }
                SuperSubAgentPromptStep::Model => {
                    let catalog = effective_provider_model_catalog(config.provider);
                    let title = if catalog.is_degraded() {
                        format!(
                            "Sub-agent model ({} account catalog degraded; available models shown)",
                            provider_display_name(config.provider)
                        )
                    } else {
                        "Sub-agent model".to_string()
                    };
                    config.model = super::super_main_prompt::prompt_super_model(
                        &title,
                        config.provider,
                        config.model.as_deref(),
                        catalog.model_ids(),
                    )?;
                }
                SuperSubAgentPromptStep::ReasoningEffort => {
                    config.model_reasoning_effort =
                        super::super_main_prompt::prompt_super_reasoning_effort(
                            "Sub-agent reasoning effort",
                            config.provider,
                            config.model.as_deref(),
                            config.model_reasoning_effort,
                        )?;
                }
                SuperSubAgentPromptStep::MaxConcurrency => {
                    config.max_concurrency = prompt_super_sub_agent_max_concurrency()?;
                }
            }
            Ok(())
        },
    )
}

pub(super) fn run_super_sub_agent_prompt_steps(
    mut config: SubAgentConfig,
    provider_explicit: bool,
    model_explicit: bool,
    effort_explicit: bool,
    mut prompt: impl FnMut(SuperSubAgentPromptStep, &mut SubAgentConfig) -> Result<()>,
) -> Result<SubAgentConfig> {
    if super_sub_agent_prompt_steps(&config, provider_explicit, true, true)
        .contains(&SuperSubAgentPromptStep::Provider)
    {
        prompt(SuperSubAgentPromptStep::Provider, &mut config)?;
    }
    for step in super_sub_agent_prompt_steps(&config, true, model_explicit, effort_explicit) {
        prompt(step, &mut config)?;
    }
    Ok(config)
}

fn prompt_super_sub_agent_max_concurrency() -> Result<SubAgentMaxConcurrency> {
    prompt_super_sub_agent_max_concurrency_with(
        |choices| prompt_super_choice("Maximum active sub-agents", choices, 0, false),
        prompt_super_sub_agent_custom_concurrency,
    )
}

fn prompt_super_sub_agent_max_concurrency_with(
    mut select: impl FnMut(&[String]) -> Result<usize>,
    mut prompt_custom: impl FnMut() -> Result<Option<SubAgentMaxConcurrency>>,
) -> Result<SubAgentMaxConcurrency> {
    let choices = super_sub_agent_concurrency_choices();
    loop {
        let index = select(&choices)?;
        let (_, selected) = prodex_mojo_core::rich::super_expose_concurrency_plan(
            DEFAULT_SUB_AGENT_MAX_CONCURRENCY,
            &prodex_cli::SUB_AGENT_MAX_CONCURRENCY_PRESETS,
            HARD_MAX_SUB_AGENT_CONCURRENCY,
            Some(index),
        )
        .expect("Mojo Super expose concurrency planner returned invalid output");
        match selected.expect("selected concurrency choice must be present") {
            prodex_mojo_core::rich::SuperExposeConcurrencyChoice::Default => {
                return Ok(SubAgentMaxConcurrency::default());
            }
            prodex_mojo_core::rich::SuperExposeConcurrencyChoice::Preset(value) => {
                return SubAgentMaxConcurrency::new(value, SubAgentConcurrencySource::Preset)
                    .map_err(anyhow::Error::msg);
            }
            prodex_mojo_core::rich::SuperExposeConcurrencyChoice::Custom => {
                if let Some(limit) = prompt_custom()? {
                    return Ok(limit);
                }
            }
        }
    }
}

pub(super) fn super_sub_agent_concurrency_choices() -> Vec<String> {
    prodex_mojo_core::rich::super_expose_concurrency_plan(
        DEFAULT_SUB_AGENT_MAX_CONCURRENCY,
        &prodex_cli::SUB_AGENT_MAX_CONCURRENCY_PRESETS,
        HARD_MAX_SUB_AGENT_CONCURRENCY,
        None,
    )
    .expect("Mojo Super expose concurrency planner returned invalid output")
    .0
    .into_iter()
    .map(|choice| match choice {
        prodex_mojo_core::rich::SuperExposeConcurrencyChoice::Default => {
            prodex_mojo_core::launch::super_prompt_format_label(
                SuperPromptLabelKind::DefaultConcurrency,
                DEFAULT_SUB_AGENT_MAX_CONCURRENCY,
                None,
                false,
            )
            .expect("Mojo Super concurrency label formatter returned invalid output")
        }
        prodex_mojo_core::rich::SuperExposeConcurrencyChoice::Preset(value) => {
            prodex_mojo_core::launch::super_prompt_format_label(
                SuperPromptLabelKind::PresetConcurrency,
                value,
                None,
                false,
            )
            .expect("Mojo Super concurrency label formatter returned invalid output")
        }
        prodex_mojo_core::rich::SuperExposeConcurrencyChoice::Custom => {
            prodex_mojo_core::launch::super_prompt_format_label(
                SuperPromptLabelKind::CustomConcurrency,
                0,
                None,
                false,
            )
            .expect("Mojo Super concurrency label formatter returned invalid output")
        }
    })
    .collect()
}

fn prompt_super_sub_agent_custom_concurrency() -> Result<Option<SubAgentMaxConcurrency>> {
    prompt_super_text_input(
        &prodex_mojo_core::launch::super_prompt_format_label(
            SuperPromptLabelKind::CustomConcurrencyTitle,
            HARD_MAX_SUB_AGENT_CONCURRENCY,
            None,
            false,
        )
        .expect("Mojo Super concurrency prompt-title formatter returned invalid output"),
        "",
        true,
        |value| value.parse::<SubAgentMaxConcurrency>(),
    )
}

pub(super) fn prompt_ping_choice(
    title: &str,
    choices: &[String],
    selected: usize,
) -> Result<usize> {
    prompt_choice(
        "Prodex Ping",
        "OpenAI ping selection TUI",
        title,
        choices,
        selected,
        false,
    )
}

fn prompt_choice(
    brand: &str,
    terminal_context: &str,
    title: &str,
    choices: &[String],
    selected: usize,
    escape_selects_last: bool,
) -> Result<usize> {
    if choices.is_empty() {
        bail!("prompt has no choices");
    }
    let mut tui = terminal_ui::AlternateScreenTerminal::stderr(terminal_context)?;
    let mut selected =
        prodex_mojo_core::launch::super_choice_initial_index(selected, choices.len())
            .expect("Mojo Super choice initial selection policy returned invalid output");
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
                Span::styled(brand, tui_title_style()),
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
            let mut footer_spans = vec![
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
            ];
            if visible.start > 0 {
                footer_spans.push(Span::styled("  ↑ more", tui_hint_style()));
            }
            if visible.end < choices.len() {
                footer_spans.push(Span::styled("  ↓ more", tui_hint_style()));
            }
            let footer = Paragraph::new(Line::from(footer_spans))
                .block(tui_connected_footer_block(tui_border_style()));
            frame.render_widget(footer, chunks[2]);
        })?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && let Some(choice) = update_super_choice_selection(
                key,
                &mut selected,
                choices.len(),
                escape_selects_last,
            )?
        {
            return Ok(choice);
        }
    }
}

fn update_super_choice_selection(
    key: crossterm::event::KeyEvent,
    selected: &mut usize,
    len: usize,
    escape_selects_last: bool,
) -> Result<Option<usize>> {
    let (key_kind, character) = match key.code {
        KeyCode::Up => (SuperPromptChoiceKey::Up, None),
        KeyCode::Down => (SuperPromptChoiceKey::Down, None),
        KeyCode::PageUp => (SuperPromptChoiceKey::PageUp, None),
        KeyCode::PageDown => (SuperPromptChoiceKey::PageDown, None),
        KeyCode::Home => (SuperPromptChoiceKey::Home, None),
        KeyCode::End => (SuperPromptChoiceKey::End, None),
        KeyCode::Enter => (SuperPromptChoiceKey::Enter, None),
        KeyCode::Esc => (SuperPromptChoiceKey::Escape, None),
        KeyCode::Char(character) => (SuperPromptChoiceKey::Character, Some(character)),
        _ => (SuperPromptChoiceKey::Other, None),
    };
    let (next_selected, action, choice) = prodex_mojo_core::launch::super_prompt_choice_event_plan(
        key_kind,
        character,
        key.modifiers.contains(KeyModifiers::CONTROL),
        *selected,
        len,
        escape_selects_last,
    )
    .expect("Mojo Super choice policy returned invalid output");
    *selected = next_selected;
    match action {
        0 => Ok(None),
        1 => Ok(choice),
        2 => bail!("Prodex Super prompt cancelled"),
        _ => unreachable!("validated Mojo Super choice action"),
    }
}

pub(super) fn visible_choice_range(
    selected: usize,
    len: usize,
    height: u16,
) -> std::ops::Range<usize> {
    prodex_mojo_core::launch::super_choice_visible_range(selected, len, usize::from(height))
        .expect("Mojo Super choice window policy returned invalid output")
}

pub(super) fn bounded_tui_text(value: &str, width: u16) -> String {
    fit_cell(value, usize::from(width).max(1))
}

pub(super) fn prompt_super_text(title: &str, initial: &str) -> Result<String> {
    prompt_super_text_input(title, initial, false, |value| {
        (!value.trim().is_empty())
            .then(|| value.to_string())
            .ok_or_else(|| "value must be nonempty".to_string())
    })?
    .ok_or_else(|| anyhow::anyhow!("Prodex Super prompt cancelled"))
}

fn super_prompt_text_edit_plan(
    key: KeyEvent,
    current_characters: usize,
    escape_returns_none: bool,
) -> Result<(SuperPromptTextAction, Option<char>, bool)> {
    let (key_kind, character) = match key.code {
        KeyCode::Char(character) => (SuperPromptTextKey::Character, Some(character)),
        KeyCode::Backspace => (SuperPromptTextKey::Backspace, None),
        KeyCode::Enter => (SuperPromptTextKey::Enter, None),
        KeyCode::Esc => (SuperPromptTextKey::Escape, None),
        _ => (SuperPromptTextKey::Other, None),
    };
    Ok(prodex_mojo_core::launch::super_prompt_text_event_plan(
        key_kind,
        character,
        key.modifiers.contains(KeyModifiers::CONTROL),
        current_characters,
        escape_returns_none,
    )
    .expect("Mojo Super text-edit policy returned invalid output"))
}

fn prompt_super_text_input<T>(
    title: &str,
    initial: &str,
    escape_returns_none: bool,
    parse: impl Fn(&str) -> std::result::Result<T, String>,
) -> Result<Option<T>> {
    let mut tui = terminal_ui::AlternateScreenTerminal::stderr("Super sub-agent text prompt TUI")?;
    let initial_prefix = prodex_mojo_core::launch::super_prompt_text_prefix_length(initial)
        .expect("Mojo Super text-prefix policy returned invalid output");
    let mut value = initial
        .get(..initial_prefix)
        .expect("Mojo Super text-prefix policy returned a non-UTF-8 boundary")
        .to_string();
    let mut validation_error = None::<String>;
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
            let mut lines = vec![Line::from(vec![
                Span::styled(value_display, tui_primary_style()),
                Span::styled("▌", tui_success_style()),
            ])];
            if let Some(error) = validation_error.as_deref() {
                lines.push(Line::from(Span::styled(
                    bounded_tui_text(error, frame.area().width.saturating_sub(4)),
                    tui_hint_style(),
                )));
            }
            let body = Paragraph::new(lines)
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
                Span::raw(if escape_returns_none {
                    " back"
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
            let (action, character, clear_validation) =
                super_prompt_text_edit_plan(key, value.chars().count(), escape_returns_none)?;
            match action {
                SuperPromptTextAction::Accept => match parse(&value) {
                    Ok(parsed) => return Ok(Some(parsed)),
                    Err(error) => validation_error = Some(error),
                },
                SuperPromptTextAction::Backspace => {
                    value.pop();
                    if clear_validation {
                        validation_error = None;
                    }
                }
                SuperPromptTextAction::Append => {
                    value.push(character.expect("Mojo append action must contain a character"));
                    if clear_validation {
                        validation_error = None;
                    }
                }
                SuperPromptTextAction::ReturnNone => return Ok(None),
                SuperPromptTextAction::Cancel => bail!("Prodex Super prompt cancelled"),
                SuperPromptTextAction::Ignore => {}
            }
        }
    }
}
pub(crate) fn prompt_super_presidio_opt_in() -> Result<bool> {
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
#[path = "super_prompt/tests.rs"]
mod super_choice_policy_tests;
