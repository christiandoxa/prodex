use super::*;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io::{IsTerminal, Read as IoRead};
use terminal_ui::{
    fit_cell, tui_border_style, tui_connected_footer_block, tui_connected_header_block,
    tui_detail_style, tui_hint_style, tui_primary_style, tui_secondary_style, tui_success_style,
    tui_title_style,
};

const SUPER_PROMPT_MAX_TEXT_CHARS: usize = 256;
const SUPER_CONFIGURED_MODEL_CATALOG_MAX_BYTES: u64 = 1024 * 1024;
const SUPER_CONFIGURED_MODEL_PROFILE_LIMIT: usize = 128;
const SUPER_CONFIGURED_MODEL_LIMIT: usize = prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT;

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedMainAgentConfig {
    provider: prodex_provider_core::ProviderId,
    model: Option<String>,
    local_url: Option<String>,
}

pub(super) fn handle_super(mut args: SuperArgs) -> Result<()> {
    args.validate_urls().map_err(anyhow::Error::msg)?;
    reject_sub_agent_recursion_reenable(&args)?;
    let stored_presidio = if args.presidio_preference().is_none() {
        stored_presidio_preference()?
    } else {
        None
    };
    let interactive = super_prompt_is_interactive();
    let (use_presidio, _main_agent, sub_agent) = resolve_super_launch_decisions_with_prompts(
        &mut args,
        interactive,
        || match stored_presidio {
            Some(use_presidio) => Ok(use_presidio),
            None => prompt_super_presidio_opt_in(),
        },
        |args| runtime_launch::runtime_resume_provider_from_codex_args(&args.codex_args),
        prompt_super_main_agent_configuration,
        prompt_super_sub_agent_configuration,
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
    args: &mut SuperArgs,
    interactive: bool,
    prompt_presidio: impl FnOnce() -> Result<bool>,
    resolve_session_provider: impl FnOnce(
        &SuperArgs,
    ) -> Result<Option<prodex_provider_core::ProviderId>>,
    prompt_main_agent: impl FnOnce(
        &SuperArgs,
        Option<prodex_provider_core::ProviderId>,
    ) -> Result<ResolvedMainAgentConfig>,
    prompt_sub_agent: impl FnOnce(&SuperArgs) -> Result<Option<SubAgentConfig>>,
) -> Result<(bool, ResolvedMainAgentConfig, Option<ResolvedSuperSubAgent>)> {
    let use_presidio = if matches!(args.cli, Some(SuperCliAgent::Kiro | SuperCliAgent::Agy)) {
        false
    } else {
        match args.presidio_preference() {
            Some(use_presidio) => use_presidio,
            None if interactive => prompt_presidio()?,
            None => false,
        }
    };
    let session_provider = resolve_session_provider(args)?;
    let main_agent = resolve_super_main_agent_with_prompt(
        args,
        interactive,
        session_provider,
        prompt_main_agent,
    )?;
    let mut sub_agent =
        resolve_super_sub_agent_with_prompt(args, interactive, || prompt_sub_agent(args))?;
    if let Some(sub_agent) = sub_agent.as_mut() {
        sub_agent.presidio_enabled = use_presidio;
    }
    Ok((use_presidio, main_agent, sub_agent))
}

fn resolve_super_main_agent_with_prompt(
    args: &mut SuperArgs,
    interactive: bool,
    session_provider: Option<prodex_provider_core::ProviderId>,
    prompt: impl FnOnce(
        &SuperArgs,
        Option<prodex_provider_core::ProviderId>,
    ) -> Result<ResolvedMainAgentConfig>,
) -> Result<ResolvedMainAgentConfig> {
    let explicit_super_provider = args
        .url
        .as_ref()
        .map(|_| prodex_provider_core::ProviderId::Local)
        .or(args.provider.map(SuperExternalProvider::provider_id))
        .or(match args.cli {
            Some(SuperCliAgent::Gemini | SuperCliAgent::Agy) => {
                Some(prodex_provider_core::ProviderId::Gemini)
            }
            Some(SuperCliAgent::Copilot) => Some(prodex_provider_core::ProviderId::Copilot),
            Some(SuperCliAgent::Kiro) => Some(prodex_provider_core::ProviderId::Kiro),
            Some(SuperCliAgent::Codex) | None => None,
        });
    let explicit_codex_provider = codex_cli_config_override_value(
        &args.codex_args,
        "model_provider",
    )
    .map(|provider| {
        prodex_provider_core::provider_implementation_registry()
            .resolve_model_provider_id(&provider)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "explicit Codex model_provider is unsupported by Prodex Super; use a canonical --provider, --url, or start a plain Codex launch"
                )
            })
    })
    .transpose()?;
    if let (Some(super_provider), Some(codex_provider)) =
        (explicit_super_provider, explicit_codex_provider)
        && super_provider != codex_provider
    {
        bail!(
            "conflicting main-agent provider inputs; remove either the Super provider option or the Codex model_provider override"
        );
    }
    let explicit_provider = explicit_super_provider.or(explicit_codex_provider);
    if let (Some(explicit), Some(bound)) = (explicit_provider, session_provider)
        && explicit != bound
    {
        bail!(
            "resumed session is bound to provider {}; remove the conflicting {} provider option or resume a matching session",
            provider_display_name(bound),
            provider_display_name(explicit)
        );
    }

    let resolved = resolve_main_agent_config(
        args,
        interactive,
        session_provider,
        explicit_provider,
        prompt,
    )?;
    apply_resolved_main_agent(args, &resolved)?;
    Ok(resolved)
}

fn resolve_main_agent_config(
    args: &SuperArgs,
    interactive: bool,
    session_provider: Option<prodex_provider_core::ProviderId>,
    explicit_provider: Option<prodex_provider_core::ProviderId>,
    prompt: impl FnOnce(
        &SuperArgs,
        Option<prodex_provider_core::ProviderId>,
    ) -> Result<ResolvedMainAgentConfig>,
) -> Result<ResolvedMainAgentConfig> {
    if let Some(provider) = session_provider {
        if interactive && explicit_provider.is_none() {
            let displayed = prompt(args, Some(provider))?;
            if displayed.provider != provider {
                bail!("resumed session provider display returned a conflicting provider");
            }
            Ok(displayed)
        } else {
            Ok(ResolvedMainAgentConfig {
                provider,
                model: args.local_model.clone(),
                local_url: args.url.clone(),
            })
        }
    } else if let Some(url) = args.url.clone() {
        Ok(ResolvedMainAgentConfig {
            provider: prodex_provider_core::ProviderId::Local,
            model: args.local_model.clone(),
            local_url: Some(url),
        })
    } else if let Some(provider) = args.provider {
        Ok(ResolvedMainAgentConfig {
            provider: provider.provider_id(),
            model: args.local_model.clone(),
            local_url: None,
        })
    } else if let Some(provider) = explicit_provider {
        Ok(ResolvedMainAgentConfig {
            provider,
            model: args.local_model.clone(),
            local_url: None,
        })
    } else if interactive {
        prompt(args, None)
    } else {
        Ok(ResolvedMainAgentConfig {
            provider: prodex_provider_core::ProviderId::OpenAi,
            model: args.local_model.clone(),
            local_url: None,
        })
    }
}

fn apply_resolved_main_agent(
    args: &mut SuperArgs,
    resolved: &ResolvedMainAgentConfig,
) -> Result<()> {
    match resolved.provider {
        prodex_provider_core::ProviderId::OpenAi => {
            if codex_cli_config_override_value(&args.codex_args, "model_provider").is_none() {
                args.codex_args.splice(
                    0..0,
                    [
                        OsString::from("-c"),
                        OsString::from("model_provider=\"openai\""),
                    ],
                );
            }
        }
        prodex_provider_core::ProviderId::Local => {
            let url = resolved
                .local_url
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("local main-agent provider requires --url"))?;
            prodex_cli::parse_super_local_url(url).map_err(anyhow::Error::msg)?;
            args.url = Some(url.to_string());
        }
        provider => {
            args.provider = SuperExternalProvider::from_provider_id(provider);
        }
    }
    Ok(())
}

fn prompt_super_main_agent_configuration(
    args: &SuperArgs,
    locked_provider: Option<prodex_provider_core::ProviderId>,
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
    let provider = providers[prompt_super_choice("Main-agent provider", &choices, 0, false)?];
    let local_url = if provider == prodex_provider_core::ProviderId::Local {
        Some(prompt_super_text(
            "Main-agent local URL",
            args.url.as_deref().unwrap_or("http://127.0.0.1:11434/v1"),
        )?)
    } else {
        None
    };
    Ok(ResolvedMainAgentConfig {
        provider,
        model: args.local_model.clone(),
        local_url,
    })
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

fn prompt_super_sub_agent_configuration(_args: &SuperArgs) -> Result<Option<SubAgentConfig>> {
    if !prompt_super_sub_agent_opt_in()? {
        return Ok(None);
    }
    prompt_super_sub_agent_config(SubAgentConfig::default(), false, false, false).map(Some)
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
        "Use sub-agents?",
        &["yes".to_string(), "no".to_string()],
        1,
        true,
    )? == 0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SuperSubAgentPromptStep {
    Provider,
    LocalUrl,
    Model,
    ReasoningEffort,
    MaxConcurrency,
}

fn super_sub_agent_prompt_steps(
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
                    let configured_models = configured_sub_agent_models(config.provider);
                    let models = if configured_models.is_empty() {
                        canonical_sub_agent_model_choices(config.provider, config.model.as_deref())
                    } else {
                        prodex_provider_core::resolve_provider_model_choices(
                            config.provider,
                            &configured_models,
                            config.model.as_deref(),
                        )
                    };
                    let choices = models
                        .iter()
                        .map(|choice| match choice {
                            prodex_provider_core::ProviderModelChoice::ProviderDefault => {
                                "provider default".to_string()
                            }
                            prodex_provider_core::ProviderModelChoice::Model(model) => {
                                model.clone()
                            }
                            prodex_provider_core::ProviderModelChoice::Custom => {
                                "custom model...".to_string()
                            }
                        })
                        .collect::<Vec<_>>();
                    let selected = config
                        .model
                        .as_ref()
                        .and_then(|model| choices.iter().position(|choice| choice == model))
                        .unwrap_or(0);
                    let selected =
                        prompt_super_choice("Sub-agent model", &choices, selected, false)?;
                    match &models[selected] {
                        prodex_provider_core::ProviderModelChoice::ProviderDefault => {
                            config.model = None
                        }
                        prodex_provider_core::ProviderModelChoice::Model(model) => {
                            config.model = Some(model.clone())
                        }
                        prodex_provider_core::ProviderModelChoice::Custom => {
                            config.model = Some(prompt_super_text(
                                "Custom sub-agent model",
                                config.model.as_deref().unwrap_or_default(),
                            )?)
                        }
                    }
                }
                SuperSubAgentPromptStep::ReasoningEffort => {
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
                    config.model_reasoning_effort = efforts[prompt_super_choice(
                        "Sub-agent reasoning effort",
                        &choices,
                        selected,
                        false,
                    )?]
                    .1;
                }
                SuperSubAgentPromptStep::MaxConcurrency => {
                    config.max_concurrency = prompt_super_sub_agent_max_concurrency()?;
                }
            }
            Ok(())
        },
    )
}

fn run_super_sub_agent_prompt_steps(
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

fn configured_sub_agent_models(provider: prodex_provider_core::ProviderId) -> Vec<String> {
    let catalog_file = match provider {
        prodex_provider_core::ProviderId::Copilot => COPILOT_RUNTIME_MODEL_CATALOG_FILE,
        prodex_provider_core::ProviderId::Kiro => KIRO_MODEL_CATALOG_FILE,
        _ => return Vec::new(),
    };
    let Ok(paths) = AppPaths::discover() else {
        return Vec::new();
    };
    let Ok(state) = AppState::load(&paths) else {
        return Vec::new();
    };
    let model_limit = SUPER_CONFIGURED_MODEL_LIMIT
        .saturating_sub(prodex_provider_core::provider_model_catalog_json(provider).len());
    let mut models = Vec::new();
    for profile in state
        .profiles
        .values()
        .take(SUPER_CONFIGURED_MODEL_PROFILE_LIMIT)
    {
        let matches_provider = matches!(
            (&profile.provider, provider),
            (
                ProfileProvider::Copilot { .. },
                prodex_provider_core::ProviderId::Copilot
            ) | (
                ProfileProvider::Kiro { .. },
                prodex_provider_core::ProviderId::Kiro
            )
        );
        if !matches_provider {
            continue;
        }
        let Ok(file) = fs::File::open(profile.codex_home.join(catalog_file)) else {
            continue;
        };
        let mut contents = String::new();
        let mut bounded = IoRead::take(file, SUPER_CONFIGURED_MODEL_CATALOG_MAX_BYTES + 1);
        if IoRead::read_to_string(&mut bounded, &mut contents).is_err()
            || contents.len() as u64 > SUPER_CONFIGURED_MODEL_CATALOG_MAX_BYTES
        {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
            continue;
        };
        configured_sub_agent_model_ids(&value, &mut models, model_limit);
        if models.len() >= model_limit {
            models.truncate(model_limit);
            break;
        }
    }
    models
}

fn configured_sub_agent_model_ids(
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
        let Some(id) = entry
            .get("id")
            .or_else(|| entry.get("slug"))
            .or_else(|| entry.get("model"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if !id.trim().is_empty() && seen.insert(id.trim().to_ascii_lowercase()) {
            models.push(id.to_string());
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

fn super_sub_agent_concurrency_choices() -> Vec<String> {
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

fn prompt_super_choice(
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
        ResolvedMainAgentConfig, SUPER_CONFIGURED_MODEL_LIMIT, SuperSubAgentPromptStep,
        bounded_tui_text, configured_sub_agent_model_ids,
        resolve_super_launch_decisions_with_prompts, resolve_super_sub_agent,
        run_super_sub_agent_prompt_steps, stored_presidio_preference,
        super_sub_agent_concurrency_choices, super_sub_agent_prompt_steps, visible_choice_range,
    };
    use prodex_cli::{SubAgentConfig, SubAgentReasoningEffort, SuperLaunchTarget};
    use prodex_provider_core::ProviderId;
    use std::cell::RefCell;
    use std::fs;

    const SESSION_ID: &str = "00000000-0000-7000-8000-000000000042";

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
        assert_eq!(visible_choice_range(3, 6, 10), 0..6);
        assert_eq!(visible_choice_range(0, 20, 4), 0..4);
        assert_eq!(visible_choice_range(19, 20, 4), 16..20);
        assert_eq!(visible_choice_range(10, 20, 4), 8..12);
        assert_eq!(visible_choice_range(10, 20, 1), 10..11);
        assert_eq!(visible_choice_range(19, 20, 30), 0..20);
    }

    #[test]
    fn concurrency_menu_uses_exact_bounded_presets_with_default_first() {
        assert_eq!(
            super_sub_agent_concurrency_choices(),
            ["default (4)", "4", "8", "16", "32", "custom..."]
        );
    }

    #[test]
    fn child_configuration_steps_are_ordered_and_local_url_is_conditional() {
        assert_eq!(
            super_sub_agent_prompt_steps(&SubAgentConfig::default(), false, false, false),
            [
                SuperSubAgentPromptStep::Provider,
                SuperSubAgentPromptStep::Model,
                SuperSubAgentPromptStep::ReasoningEffort,
                SuperSubAgentPromptStep::MaxConcurrency,
            ]
        );
        assert_eq!(
            super_sub_agent_prompt_steps(
                &SubAgentConfig {
                    provider: ProviderId::Local,
                    ..SubAgentConfig::default()
                },
                false,
                false,
                false,
            ),
            [
                SuperSubAgentPromptStep::Provider,
                SuperSubAgentPromptStep::LocalUrl,
                SuperSubAgentPromptStep::Model,
                SuperSubAgentPromptStep::ReasoningEffort,
                SuperSubAgentPromptStep::MaxConcurrency,
            ]
        );
    }

    #[test]
    fn launch_sequence_combines_outer_decisions_and_child_runner() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(super::SUB_AGENT_RECURSION_MARKER);
        let calls = RefCell::new(Vec::new());
        let mut args = super_args(&[]);
        let (_, _, enabled) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            true,
            || {
                calls.borrow_mut().push("Presidio");
                Ok(true)
            },
            |_| Ok(None),
            |_, locked| {
                assert_eq!(locked, None);
                calls.borrow_mut().push("Main-agent provider");
                Ok(ResolvedMainAgentConfig {
                    provider: ProviderId::OpenAi,
                    model: None,
                    local_url: None,
                })
            },
            |_| {
                calls.borrow_mut().push("Use sub-agents?");
                let config = run_super_sub_agent_prompt_steps(
                    SubAgentConfig::default(),
                    false,
                    false,
                    false,
                    |step, config| {
                        calls.borrow_mut().push(match step {
                            SuperSubAgentPromptStep::Provider => "Sub-agent provider",
                            SuperSubAgentPromptStep::LocalUrl => "Sub-agent local URL",
                            SuperSubAgentPromptStep::Model => "Sub-agent model",
                            SuperSubAgentPromptStep::ReasoningEffort => {
                                "Sub-agent reasoning effort"
                            }
                            SuperSubAgentPromptStep::MaxConcurrency => "Maximum active sub-agents",
                        });
                        match step {
                            SuperSubAgentPromptStep::Provider => {
                                config.provider = ProviderId::Local
                            }
                            SuperSubAgentPromptStep::LocalUrl => {
                                config.url = Some("http://127.0.0.1:8131/v1".to_string())
                            }
                            _ => {}
                        }
                        Ok(())
                    },
                )?;
                Ok(Some(config))
            },
        )
        .unwrap();
        assert!(enabled.is_some());
        assert_eq!(
            &*calls.borrow(),
            &[
                "Presidio",
                "Main-agent provider",
                "Use sub-agents?",
                "Sub-agent provider",
                "Sub-agent local URL",
                "Sub-agent model",
                "Sub-agent reasoning effort",
                "Maximum active sub-agents",
            ]
        );

        let disabled_calls = RefCell::new(Vec::new());
        let mut args = super_args(&[]);
        let (_, _, disabled) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            true,
            || {
                disabled_calls.borrow_mut().push("Presidio");
                Ok(false)
            },
            |_| Ok(None),
            |_, locked| {
                assert_eq!(locked, None);
                disabled_calls.borrow_mut().push("Main-agent provider");
                Ok(ResolvedMainAgentConfig {
                    provider: ProviderId::OpenAi,
                    model: None,
                    local_url: None,
                })
            },
            |_| {
                disabled_calls.borrow_mut().push("Use sub-agents?");
                Ok(None)
            },
        )
        .unwrap();
        assert!(disabled.is_none());
        assert_eq!(
            &*disabled_calls.borrow(),
            &["Presidio", "Main-agent provider", "Use sub-agents?"]
        );
    }

    #[test]
    fn configured_model_ids_augment_the_canonical_picker_inputs() {
        let mut configured = Vec::new();
        configured_sub_agent_model_ids(
            &serde_json::json!({
                "models": [
                    {"id": "profile-only-model"},
                    {"slug": "slug-only-model"},
                    {"model": "model-only-model"},
                    {"id": "   "}
                ]
            }),
            &mut configured,
            SUPER_CONFIGURED_MODEL_LIMIT,
        );
        let choices = prodex_provider_core::resolve_provider_model_choices(
            ProviderId::Kiro,
            &configured,
            None,
        );
        for expected in ["profile-only-model", "slug-only-model", "model-only-model"] {
            assert!(choices.iter().any(|choice| matches!(
                choice,
                prodex_provider_core::ProviderModelChoice::Model(model) if model == expected
            )));
        }
    }

    #[test]
    fn configured_model_budget_counts_only_valid_unique_ids() {
        let mut configured = vec!["duplicate".to_string()];
        configured_sub_agent_model_ids(
            &serde_json::json!({
                "models": [
                    {},
                    {"id": "duplicate"},
                    {"id": "DUPLICATE"},
                    {"id": "tail-model"}
                ]
            }),
            &mut configured,
            2,
        );

        assert_eq!(configured, ["duplicate", "tail-model"]);
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
        let mut args = super_args(&[SESSION_ID]);
        let calls = RefCell::new(Vec::new());
        let (presidio, main_agent, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            true,
            || {
                calls.borrow_mut().push("presidio");
                Ok(true)
            },
            |_| Ok(None),
            |_, locked| {
                assert_eq!(locked, None);
                calls.borrow_mut().push("main-agent");
                Ok(ResolvedMainAgentConfig {
                    provider: ProviderId::OpenAi,
                    model: None,
                    local_url: None,
                })
            },
            |_| {
                calls.borrow_mut().push("sub-agent");
                Ok(Some(SubAgentConfig {
                    provider: ProviderId::Kiro,
                    model: Some("gpt-5.6-luna".to_string()),
                    model_reasoning_effort: Some(SubAgentReasoningEffort::Max),
                    url: None,
                    max_concurrency: Default::default(),
                }))
            },
        )
        .expect("interactive resolution should succeed");

        assert_eq!(&*calls.borrow(), &["presidio", "main-agent", "sub-agent"]);
        assert!(presidio);
        assert_eq!(main_agent.provider, ProviderId::OpenAi);
        assert_eq!(
            super::codex_cli_config_override_value(&args.codex_args, "model_provider").as_deref(),
            Some("openai")
        );
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
        let mut args = super_args(&[SESSION_ID]);
        let (presidio, main_agent, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            true,
            || {
                calls.borrow_mut().push("presidio");
                Ok(false)
            },
            |_| Ok(None),
            |_, locked| {
                assert_eq!(locked, None);
                calls.borrow_mut().push("main-agent");
                Ok(ResolvedMainAgentConfig {
                    provider: ProviderId::Kiro,
                    model: None,
                    local_url: None,
                })
            },
            |_| {
                calls.borrow_mut().push("sub-agent");
                Ok(None)
            },
        )
        .expect("interactive skip should succeed");

        assert_eq!(&*calls.borrow(), &["presidio", "main-agent", "sub-agent"]);
        assert!(!presidio);
        assert_eq!(main_agent.provider, ProviderId::Kiro);
        assert!(sub_agent.is_none());
    }

    #[test]
    fn pure_resolution_skips_prompts_for_non_tty_and_fully_configured_resume() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(super::SUB_AGENT_RECURSION_MARKER);
        let mut args = super_args(&[]);
        let (presidio, main_agent, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            false,
            || panic!("non-TTY Presidio prompt must not run"),
            |_| Ok(None),
            |_, _| panic!("non-TTY main-agent prompt must not run"),
            |_| panic!("non-TTY sub-agent prompt must not run"),
        )
        .expect("non-TTY defaults should resolve");
        assert!(!presidio);
        assert_eq!(main_agent.provider, ProviderId::OpenAi);
        assert!(sub_agent.is_none());

        let mut args = super_args(&[
            "--presidio",
            "--provider",
            "copilot",
            "--sub-agent",
            "--sub-agent-provider",
            "copilot",
            "--sub-agent-model",
            "configured-model",
            "--sub-agent-model-reasoning-effort",
            "xhigh",
            SESSION_ID,
        ]);
        let (presidio, main_agent, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            true,
            || panic!("explicit Presidio must skip the prompt"),
            |_| Ok(None),
            |_, _| panic!("explicit main provider must skip the prompt"),
            |_| panic!("explicit sub-agent config must skip the wizard"),
        )
        .expect("explicit resume should resolve");
        assert!(presidio);
        assert_eq!(main_agent.provider, ProviderId::Copilot);
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
    fn persisted_presidio_preference_is_used_until_cli_disable_overrides_it() {
        let root = std::env::temp_dir().join(format!(
            "prodex-super-presidio-preference-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).expect("preference root should exist");
        fs::write(root.join("presidio.toml"), "enabled = true\n")
            .expect("preference should be written");
        let _home = crate::test_support::TestEnvVarGuard::set(
            "PRODEX_HOME",
            root.to_str().expect("preference root should be UTF-8"),
        );
        let persisted = stored_presidio_preference()
            .expect("persisted preference should load")
            .expect("persisted preference should exist");
        assert!(persisted);

        let resolve = |args: &mut prodex_cli::SuperArgs| {
            resolve_super_launch_decisions_with_prompts(
                args,
                true,
                || Ok(persisted),
                |_| Ok(None),
                |_, _| {
                    Ok(ResolvedMainAgentConfig {
                        provider: ProviderId::OpenAi,
                        model: None,
                        local_url: None,
                    })
                },
                |_| Ok(None),
            )
            .expect("Super preference should resolve")
            .0
        };

        let mut enabled = super_args(&[]);
        assert!(resolve(&mut enabled));

        let mut disabled = super_args(&["--no-presidio"]);
        assert!(!resolve(&mut disabled));

        fs::remove_dir_all(root).expect("preference root should be removed");
    }

    #[test]
    fn resumed_provider_affinity_displays_locked_provider_and_rejects_conflicts() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(super::SUB_AGENT_RECURSION_MARKER);
        let mut args = super_args(&[SESSION_ID, "--no-sub-agent"]);
        let (_, main_agent, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            true,
            || Ok(false),
            |_| Ok(Some(ProviderId::Kiro)),
            |_, locked| {
                assert_eq!(locked, Some(ProviderId::Kiro));
                Ok(ResolvedMainAgentConfig {
                    provider: ProviderId::Kiro,
                    model: None,
                    local_url: None,
                })
            },
            |_| panic!("explicit --no-sub-agent must skip the prompt"),
        )
        .expect("bound resume should resolve");
        assert_eq!(main_agent.provider, ProviderId::Kiro);
        assert!(sub_agent.is_none());

        let mut args = super_args(&["--provider", "copilot", SESSION_ID, "--no-sub-agent"]);
        let error = resolve_super_launch_decisions_with_prompts(
            &mut args,
            false,
            || panic!("non-TTY must not prompt"),
            |_| Ok(Some(ProviderId::Kiro)),
            |_, _| panic!("conflicting provider must fail before picker"),
            |_| panic!("conflicting provider must fail before sub-agent resolution"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("bound to provider Kiro"), "{error}");
        assert!(error.contains("GitHub Copilot"), "{error}");

        let mut args = super_args(&[
            "-c",
            "model_provider=\"prodex-kiro\"",
            SESSION_ID,
            "--no-sub-agent",
        ]);
        let error = resolve_super_launch_decisions_with_prompts(
            &mut args,
            false,
            || panic!("non-TTY must not prompt"),
            |_| Ok(Some(ProviderId::OpenAi)),
            |_, _| panic!("conflicting provider must fail before picker"),
            |_| panic!("conflicting provider must fail before sub-agent resolution"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("bound to provider OpenAI"), "{error}");
        assert!(error.contains("Kiro"), "{error}");

        let mut args = super_args(&["-c", "model_provider=\"prodex-kiro\"", "--no-sub-agent"]);
        let (_, main_agent, _) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            true,
            || Ok(false),
            |_| Ok(None),
            |_, _| panic!("explicit model_provider must skip the picker"),
            |_| panic!("explicit --no-sub-agent must skip the prompt"),
        )
        .unwrap();
        assert_eq!(main_agent.provider, ProviderId::Kiro);
        assert_eq!(args.provider, Some(prodex_cli::SuperExternalProvider::Kiro));
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
