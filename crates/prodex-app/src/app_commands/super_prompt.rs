use super::ResolvedMainAgentConfig;
use crate::{
    AppPaths, AppState, AppStateIoExt, COPILOT_RUNTIME_MODEL_CATALOG_FILE, KIRO_MODEL_CATALOG_FILE,
    ProfileProvider, ResolvedSuperSubAgent, SUB_AGENT_RECURSION_MARKER, SubAgentRecursionPolicy,
    canonical_sub_agent_providers, provider_display_name, resolve_super_launch_target,
    resolve_super_sub_agent_config, sub_agent_recursion_policy,
};
use crate::{parse_kiro_model_catalog_text, read_provider_model_catalog_text};
use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use prodex_cli::{
    DEFAULT_SUB_AGENT_MAX_CONCURRENCY, HARD_MAX_SUB_AGENT_CONCURRENCY, SubAgentConfig,
    SubAgentMaxConcurrency, SubAgentPreference, SuperArgs, SuperCliAgent,
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::collections::BTreeSet;
use std::io::{self, IsTerminal};
use terminal_ui::{
    fit_cell, tui_border_style, tui_connected_footer_block, tui_connected_header_block,
    tui_detail_style, tui_hint_style, tui_primary_style, tui_secondary_style, tui_success_style,
    tui_title_style,
};

const SUPER_PROMPT_MAX_TEXT_CHARS: usize = 256;
pub(super) const SUPER_CONFIGURED_MODEL_PROFILE_LIMIT: usize = 128;
pub(super) const SUPER_CONFIGURED_MODEL_LIMIT: usize =
    prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT;

pub(super) fn prompt_super_main_agent_configuration(
    args: &SuperArgs,
    locked_provider: Option<prodex_provider_core::ProviderId>,
) -> Result<ResolvedMainAgentConfig> {
    // Ordinary Super chooses agent/provider here; model and effort come from the
    // provider-scoped remembered preference resolver.
    prompt_super_main_agent_configuration_with_options(args, locked_provider, false)
}

pub(super) fn prompt_super_main_agent_configuration_for_expose(
    args: &SuperArgs,
    locked_provider: Option<prodex_provider_core::ProviderId>,
) -> Result<ResolvedMainAgentConfig> {
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
        .map(|provider| {
            let label = provider_display_name(*provider);
            if locked_provider.is_some() {
                format!("{label} (session affinity)")
            } else {
                label.to_string()
            }
        })
        .collect::<Vec<_>>();
    let selected_provider = locked_provider
        .and_then(|provider| {
            providers
                .iter()
                .position(|candidate| *candidate == provider)
        })
        .unwrap_or(0);
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
    let mut steps = Vec::with_capacity(5);
    if !provider_explicit {
        steps.push(SuperSubAgentPromptStep::Provider);
    }
    if config.provider == prodex_provider_core::ProviderId::Local && config.url.is_none() {
        steps.push(SuperSubAgentPromptStep::LocalUrl);
    }
    if !model_explicit {
        steps.push(SuperSubAgentPromptStep::Model);
    }
    if !effort_explicit {
        steps.push(SuperSubAgentPromptStep::ReasoningEffort);
    }
    steps.push(SuperSubAgentPromptStep::MaxConcurrency);
    steps
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
                        .map(|provider| provider_display_name(*provider).to_string())
                        .collect::<Vec<_>>();
                    let selected = providers
                        .iter()
                        .position(|provider| *provider == config.provider)
                        .unwrap_or(0);
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
                    config.model = super::super_main_prompt::prompt_super_model(
                        "Sub-agent model",
                        config.provider,
                        config.model.as_deref(),
                        configured_sub_agent_models(config.provider),
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
    if !provider_explicit {
        prompt(SuperSubAgentPromptStep::Provider, &mut config)?;
    }
    for step in super_sub_agent_prompt_steps(&config, true, model_explicit, effort_explicit) {
        prompt(step, &mut config)?;
    }
    Ok(config)
}

pub(super) fn configured_sub_agent_models(
    provider: prodex_provider_core::ProviderId,
) -> Vec<String> {
    let Ok(paths) = AppPaths::discover() else {
        return Vec::new();
    };
    configured_sub_agent_models_from_paths(&paths, provider)
}

pub(super) fn configured_sub_agent_models_from_paths(
    paths: &AppPaths,
    provider: prodex_provider_core::ProviderId,
) -> Vec<String> {
    let catalog_file = match provider {
        prodex_provider_core::ProviderId::Copilot => COPILOT_RUNTIME_MODEL_CATALOG_FILE,
        prodex_provider_core::ProviderId::Kiro => KIRO_MODEL_CATALOG_FILE,
        _ => return Vec::new(),
    };
    let Ok(state) = AppState::load(paths) else {
        return Vec::new();
    };
    let model_limit = SUPER_CONFIGURED_MODEL_LIMIT
        .saturating_sub(prodex_provider_core::provider_model_catalog_json(provider).len());
    let mut models = Vec::new();
    let mut usable_catalog_count = 0;
    for profile in state.profiles.values().filter(|profile| {
        matches!(
            (&profile.provider, provider),
            (
                ProfileProvider::Copilot { .. },
                prodex_provider_core::ProviderId::Copilot
            ) | (
                ProfileProvider::Kiro { .. },
                prodex_provider_core::ProviderId::Kiro
            )
        )
    }) {
        if usable_catalog_count >= SUPER_CONFIGURED_MODEL_PROFILE_LIMIT {
            break;
        }
        let Ok(Some(contents)) =
            read_provider_model_catalog_text(&profile.codex_home.join(catalog_file))
        else {
            continue;
        };
        let value = if provider == prodex_provider_core::ProviderId::Kiro {
            let Ok(models) = parse_kiro_model_catalog_text(&contents) else {
                continue;
            };
            serde_json::json!({"models": models})
        } else {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
                continue;
            };
            value
        };
        usable_catalog_count += 1;
        configured_sub_agent_model_ids(&value, &mut models, model_limit);
        if models.len() >= model_limit {
            models.truncate(model_limit);
            break;
        }
    }
    models
}

pub(super) fn configured_sub_agent_model_ids(
    value: &serde_json::Value,
    models: &mut Vec<String>,
    model_limit: usize,
) {
    let Some(entries) = value.get("models").and_then(serde_json::Value::as_array) else {
        return;
    };
    let mut seen = models
        .iter()
        .map(|model| model.trim().to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    for entry in entries {
        if models.len() >= model_limit {
            break;
        }
        let Some(id) = ["id", "model_id", "modelId", "slug", "model"]
            .into_iter()
            .find_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
        else {
            continue;
        };
        if !id.trim().is_empty() && seen.insert(id.trim().to_ascii_lowercase()) {
            models.push(id.trim().to_string());
        }
    }
}

fn prompt_super_sub_agent_max_concurrency() -> Result<SubAgentMaxConcurrency> {
    let choices = super_sub_agent_concurrency_choices();
    loop {
        match prompt_super_choice("Maximum active sub-agents", &choices, 0, false)? {
            0 => return Ok(SubAgentMaxConcurrency::default()),
            index if index <= prodex_cli::SUB_AGENT_MAX_CONCURRENCY_PRESETS.len() => {
                return choices[index]
                    .parse::<SubAgentMaxConcurrency>()
                    .map_err(anyhow::Error::msg);
            }
            _ => {
                if let Some(limit) = prompt_super_sub_agent_custom_concurrency()? {
                    return Ok(limit);
                }
            }
        }
    }
}

pub(super) fn super_sub_agent_concurrency_choices() -> Vec<String> {
    let mut choices = vec![format!("default ({DEFAULT_SUB_AGENT_MAX_CONCURRENCY})")];
    choices.extend(
        prodex_cli::SUB_AGENT_MAX_CONCURRENCY_PRESETS
            .iter()
            .map(u16::to_string),
    );
    choices.push("custom...".to_string());
    choices
}

fn prompt_super_sub_agent_custom_concurrency() -> Result<Option<SubAgentMaxConcurrency>> {
    prompt_super_text_input(
        &format!("Enter maximum active sub-agents (1-{HARD_MAX_SUB_AGENT_CONCURRENCY})"),
        "",
        true,
        |value| value.parse::<SubAgentMaxConcurrency>(),
    )
}

pub(super) fn prompt_super_choice(
    title: &str,
    choices: &[String],
    selected: usize,
    escape_selects_last: bool,
) -> Result<usize> {
    if choices.is_empty() {
        bail!("Super prompt has no choices");
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
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => *selected = selected.checked_sub(1).unwrap_or(len - 1),
        KeyCode::Down | KeyCode::Char('j') => *selected = (*selected + 1) % len,
        KeyCode::PageUp => *selected = selected.saturating_sub(10),
        KeyCode::PageDown => *selected = selected.saturating_add(10).min(len - 1),
        KeyCode::Home => *selected = 0,
        KeyCode::End => *selected = len - 1,
        KeyCode::Enter => return Ok(Some(*selected)),
        KeyCode::Esc if escape_selects_last => return Ok(Some(len - 1)),
        KeyCode::Esc => bail!("Prodex Super prompt cancelled"),
        KeyCode::Char('c') | KeyCode::Char('z')
            if key.modifiers.contains(KeyModifiers::CONTROL) =>
        {
            bail!("Prodex Super prompt cancelled")
        }
        _ => {}
    }
    Ok(None)
}

pub(super) fn visible_choice_range(
    selected: usize,
    len: usize,
    height: u16,
) -> std::ops::Range<usize> {
    let visible = usize::from(height).max(1).min(len);
    let start = selected
        .saturating_sub(visible / 2)
        .min(len.saturating_sub(visible));
    start..start + visible
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

fn prompt_super_text_input<T>(
    title: &str,
    initial: &str,
    escape_returns_none: bool,
    parse: impl Fn(&str) -> std::result::Result<T, String>,
) -> Result<Option<T>> {
    let mut tui = terminal_ui::AlternateScreenTerminal::stderr("Super sub-agent text prompt TUI")?;
    let mut value = initial
        .chars()
        .take(SUPER_PROMPT_MAX_TEXT_CHARS)
        .collect::<String>();
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
            match key.code {
                KeyCode::Enter => match parse(&value) {
                    Ok(parsed) => return Ok(Some(parsed)),
                    Err(error) => validation_error = Some(error),
                },
                KeyCode::Backspace => {
                    value.pop();
                    validation_error = None;
                }
                KeyCode::Char(character)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && value.chars().count() < SUPER_PROMPT_MAX_TEXT_CHARS =>
                {
                    value.push(character);
                    validation_error = None;
                }
                KeyCode::Esc if escape_returns_none => return Ok(None),
                KeyCode::Esc => bail!("Prodex Super prompt cancelled"),
                KeyCode::Char('c') | KeyCode::Char('z')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    bail!("Prodex Super prompt cancelled")
                }
                _ => {}
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
