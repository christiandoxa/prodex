use super::ResolvedMainAgentConfig;
use crate::{
    ResolvedSuperSubAgent, SUB_AGENT_RECURSION_MARKER, SubAgentRecursionPolicy,
    canonical_sub_agent_providers, effective_provider_model_catalog, provider_display_name,
    resolve_super_launch_target, resolve_super_sub_agent_config, sub_agent_recursion_policy,
};
use anyhow::{Result, bail};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use prodex_cli::{
    DEFAULT_SUB_AGENT_MAX_CONCURRENCY, HARD_MAX_SUB_AGENT_CONCURRENCY, SubAgentConfig,
    SubAgentMaxConcurrency, SubAgentPreference, SuperArgs,
};
use std::io::{self, IsTerminal, Write};

const SUPER_PROMPT_MAX_TEXT_CHARS: usize = 256;

struct PresidioPromptTerminal {
    stderr: io::Stderr,
}

impl PresidioPromptTerminal {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stderr = io::stderr();
        if let Err(error) = execute!(
            stderr,
            EnterAlternateScreen,
            Hide,
            MoveTo(0, 0),
            Clear(ClearType::All)
        ) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        Ok(Self { stderr })
    }

    fn render(&mut self) -> Result<()> {
        execute!(self.stderr, MoveTo(0, 0), Clear(ClearType::All))?;
        let panel = terminal_ui::render_text_panel(
            "Presidio opt-in",
            "Use Presidio for data safety?\n\nDetected sensitive data is redacted from request bodies before upstream delivery.\n\ny enable · n skip · enter skip · esc skip",
        );
        write!(self.stderr, "{panel}")?;
        self.stderr.flush()?;
        Ok(())
    }
}

impl Drop for PresidioPromptTerminal {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.stderr, Show, LeaveAlternateScreen);
        let _ = self.stderr.flush();
    }
}

pub(super) fn prompt_super_main_agent_configuration(
    args: &SuperArgs,
    locked_provider: Option<prodex_provider_core::ProviderId>,
) -> Result<ResolvedMainAgentConfig> {
    // Ordinary Super chooses agent/provider here; model and effort come from the
    // provider-scoped remembered preference resolver.
    prompt_super_main_agent_configuration_with_options(args, locked_provider, false)
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
    if !provider_explicit {
        prompt(SuperSubAgentPromptStep::Provider, &mut config)?;
    }
    for step in super_sub_agent_prompt_steps(&config, true, model_explicit, effort_explicit) {
        prompt(step, &mut config)?;
    }
    Ok(config)
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
    let selected = selected.min(choices.len() - 1);
    loop {
        render_super_choice_prompt(title, choices, selected)?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if let Some(choice) =
            parse_super_choice_input(input.trim(), choices.len(), selected, escape_selects_last)?
        {
            return Ok(choice);
        }
        writeln!(io::stderr(), "Enter a number from 1 to {}.", choices.len())?;
    }
}

fn render_super_choice_prompt(title: &str, choices: &[String], selected: usize) -> Result<()> {
    let mut stderr = io::stderr().lock();
    writeln!(stderr, "{title}:")?;
    for (index, choice) in choices.iter().enumerate() {
        let marker = if index == selected { "*" } else { " " };
        writeln!(stderr, "  {marker} {}. {choice}", index + 1)?;
    }
    write!(stderr, "Select [{}]: ", selected + 1)?;
    stderr.flush()?;
    Ok(())
}

fn parse_super_choice_input(
    value: &str,
    choice_count: usize,
    selected: usize,
    escape_selects_last: bool,
) -> Result<Option<usize>> {
    if value.is_empty() {
        return Ok(Some(selected));
    }
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "q" | "quit" | "cancel" | "esc"
    ) {
        if escape_selects_last {
            return Ok(Some(choice_count - 1));
        }
        bail!("Prodex Super prompt cancelled");
    }
    Ok(value
        .parse::<usize>()
        .ok()
        .filter(|choice| (1..=choice_count).contains(choice))
        .map(|choice| choice - 1))
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
    loop {
        let mut stderr = io::stderr().lock();
        if initial.is_empty() {
            write!(stderr, "{title}: ")?;
        } else {
            write!(stderr, "{title} [{initial}]: ")?;
        }
        stderr.flush()?;
        drop(stderr);

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let value = input.trim();
        if escape_returns_none
            && matches!(
                value.to_ascii_lowercase().as_str(),
                "q" | "quit" | "back" | "cancel"
            )
        {
            return Ok(None);
        }
        let value = if value.is_empty() { initial } else { value };
        if value.chars().count() > SUPER_PROMPT_MAX_TEXT_CHARS {
            writeln!(io::stderr(), "value is too long")?;
            continue;
        }
        match parse(value) {
            Ok(parsed) => return Ok(Some(parsed)),
            Err(error) => writeln!(io::stderr(), "{error}")?,
        }
    }
}

pub(crate) fn prompt_super_presidio_opt_in() -> Result<bool> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Ok(false);
    }

    let mut tui = PresidioPromptTerminal::new()?;
    tui.render()?;
    loop {
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
