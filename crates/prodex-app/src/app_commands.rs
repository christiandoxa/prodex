use super::*;
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
#[cfg(test)]
mod native_cli_tests;
mod ping;
mod presidio;
mod quota;
mod redeem;
pub(crate) mod runtime_launch;
mod selection;
mod session;
mod shared;
mod status;
mod super_prompt;

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
pub(super) use self::super_prompt::prompt_super_presidio_opt_in;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ResolvedMainAgentConfig {
    provider: prodex_provider_core::ProviderId,
    model: Option<String>,
    local_url: Option<String>,
}

pub(super) fn handle_super(mut args: SuperArgs) -> Result<()> {
    args.validate_urls().map_err(anyhow::Error::msg)?;
    super_prompt::reject_sub_agent_recursion_reenable(&args)?;
    runtime_launch::resume_repair::repair_super_resume_session_metadata(&args)?;
    let interactive = super_prompt::super_prompt_is_interactive();
    let (use_presidio, _main_agent, sub_agent) = resolve_super_launch_decisions_with_prompts(
        &mut args,
        interactive,
        super_prompt::prompt_super_presidio_opt_in,
        |args| runtime_launch::runtime_resume_provider_from_codex_args(&args.codex_args),
        super_prompt::prompt_super_main_agent_configuration,
        super_prompt::prompt_super_sub_agent_configuration,
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
    let args = runtime_launch::resolved_super_runtime_tool_args(args, use_presidio);
    handle_super_runtime_tools(args, sub_agent)
}

pub(super) fn resolve_super_sub_agent(
    args: &SuperArgs,
    interactive: bool,
) -> Result<Option<ResolvedSuperSubAgent>> {
    super_prompt::resolve_super_sub_agent_with_prompt(
        args,
        interactive && super_prompt::super_prompt_is_interactive(),
        || super_prompt::prompt_super_sub_agent_configuration(args),
    )
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
    let use_presidio = if matches!(
        args.cli,
        Some(SuperCliAgent::Gemini | SuperCliAgent::Kiro | SuperCliAgent::Agy)
    ) {
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
        super_prompt::resolve_super_sub_agent_with_prompt(args, interactive, || {
            prompt_sub_agent(args)
        })?;
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
        prodex_provider_core::ProviderId::Kiro
            if args.cli == Some(SuperCliAgent::Kiro) && args.provider.is_none() => {}
        provider => {
            args.provider = SuperExternalProvider::from_provider_id(provider);
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    runtime_launch::resolve_runtime_launch_profile_name(state, requested)
}

#[cfg(test)]
mod sub_agent_prompt_tests {
    use super::super_prompt::{
        SUPER_CONFIGURED_MODEL_LIMIT, SuperSubAgentPromptStep, bounded_tui_text,
        configured_sub_agent_model_ids, run_super_sub_agent_prompt_steps,
        super_sub_agent_concurrency_choices, super_sub_agent_prompt_steps, visible_choice_range,
    };
    use super::{
        ResolvedMainAgentConfig, SUB_AGENT_RECURSION_MARKER, codex_cli_config_override_value,
        resolve_super_launch_decisions_with_prompts, resolve_super_sub_agent, runtime_launch,
    };
    use prodex_cli::{SubAgentConfig, SubAgentReasoningEffort, SuperLaunchTarget};
    use prodex_provider_core::ProviderId;
    use std::cell::RefCell;

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
                    {"id": "profile-model"},
                    {"model_id": " kiro-model "},
                    {"slug": "slug-model"},
                    {"model": "model-model"},
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
        for expected in ["profile-model", "kiro-model", "slug-model", "model-model"] {
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
    fn resolved_sub_agent_inherits_required_optional_tools() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(super::SUB_AGENT_RECURSION_MARKER);
        let command = crate::parse_cli_command_from([
            "prodex",
            "s",
            "--sub-agent",
            "--require-tool",
            "rtk",
            "--require-tool",
            "playwright",
        ])
        .expect("configured sub-agent command should parse");
        let crate::Commands::Super(args) = command else {
            panic!("expected Super command");
        };

        let resolved = resolve_super_sub_agent(&args, false)
            .expect("configured sub-agent should resolve")
            .expect("configured sub-agent should be enabled");

        assert_eq!(
            resolved.required_tools,
            vec![
                prodex_optional_tools::OptionalToolId::Rtk,
                prodex_optional_tools::OptionalToolId::PlaywrightMcp,
            ]
        );
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
        let runtime_args = runtime_launch::resolved_super_runtime_tool_args(args, false);
        assert!(codex_cli_config_override_value(&runtime_args.codex_args, "model").is_none());
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
    fn native_gemini_skips_unavailable_presidio_prompt() {
        let _marker = crate::test_support::TestEnvVarGuard::unset(SUB_AGENT_RECURSION_MARKER);
        let mut args = super_args(&["--cli", "gemini", "--provider", "gemini"]);
        let (presidio, main_agent, sub_agent) = resolve_super_launch_decisions_with_prompts(
            &mut args,
            true,
            || panic!("native Gemini must not prompt for unavailable Presidio"),
            |_| Ok(None),
            |_, _| panic!("explicit native Gemini must skip the picker"),
            |_| Ok(None),
        )
        .expect("native Gemini should resolve without Presidio");

        assert!(!presidio);
        assert_eq!(main_agent.provider, ProviderId::Gemini);
        assert!(sub_agent.is_none());
        assert!(args.provider.is_some());
        assert!(args.url.is_none());
        crate::runtime_gemini_cli::validate_super_native_cli_preflight(&args).unwrap();
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
