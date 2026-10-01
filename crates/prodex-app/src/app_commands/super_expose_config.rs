use super::{
    ResolvedMainAgentConfig, SuperPromptOrder, resolve_super_launch_decisions_with_order,
    runtime_launch, super_prompt,
};
use crate::{ResolvedSuperSubAgent, codex_cli_config_override_value};
use anyhow::Result;
use prodex_cli::{SubAgentConfig, SuperArgs};
use std::ffi::OsString;

pub(crate) fn resolve_super_expose_launch_configuration(args: &mut SuperArgs) -> Result<()> {
    let interactive = super_prompt::super_prompt_is_interactive()
        && !args.dry_run
        && !prodex_runtime_launch::prodex_dry_run_requested(&args.codex_args);
    if !interactive {
        return Ok(());
    }
    super_prompt::reject_sub_agent_recursion_reenable(args)?;
    resolve_super_expose_launch_decisions_with_prompts(
        args,
        true,
        super_prompt::prompt_super_presidio_opt_in,
        |args| runtime_launch::runtime_resume_provider_from_codex_args(&args.codex_args),
        super_prompt::prompt_super_expose_main_agent_configuration,
        super_prompt::prompt_super_sub_agent_configuration,
    )
}

fn resolve_super_expose_launch_decisions_with_prompts(
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
) -> Result<()> {
    let (use_presidio, main_agent, sub_agent) = resolve_super_launch_decisions_with_order(
        args,
        interactive,
        SuperPromptOrder::MainAgentFirst,
        prompt_presidio,
        resolve_session_provider,
        prompt_main_agent,
        prompt_sub_agent,
    )?;
    materialize_super_expose_launch_decisions(args, use_presidio, &main_agent, sub_agent.as_ref());
    Ok(())
}

fn materialize_super_expose_launch_decisions(
    args: &mut SuperArgs,
    use_presidio: bool,
    main_agent: &ResolvedMainAgentConfig,
    sub_agent: Option<&ResolvedSuperSubAgent>,
) {
    args.presidio = use_presidio;
    args.no_presidio = !use_presidio;

    if let Some(model) = main_agent.model.as_ref() {
        args.local_model = Some(model.clone());
    }
    if let Some(effort) = main_agent.reasoning_effort.as_deref()
        && codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort").as_deref()
            != Some(effort)
    {
        args.codex_args.splice(
            0..0,
            [
                OsString::from("-c"),
                OsString::from(format!(
                    "model_reasoning_effort={}",
                    crate::runtime_catalog_config::toml_string_literal(effort)
                )),
            ],
        );
    }

    match sub_agent {
        Some(sub_agent) => {
            args.sub_agent = true;
            args.no_sub_agent = false;
            args.sub_agent_provider = Some(sub_agent.provider);
            args.sub_agent_model = sub_agent.model.clone();
            args.sub_agent_model_reasoning_effort = sub_agent.effort;
            args.sub_agent_url = sub_agent.url.clone();
            args.sub_agent_max_concurrency = Some(sub_agent.max_concurrency);
        }
        None => {
            args.sub_agent = false;
            args.no_sub_agent = true;
            args.sub_agent_provider = None;
            args.sub_agent_model = None;
            args.sub_agent_model_reasoning_effort = None;
            args.sub_agent_url = None;
            args.sub_agent_max_concurrency = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_cli::SubAgentReasoningEffort;
    use prodex_provider_core::ProviderId;
    use std::cell::RefCell;

    fn super_args(values: &[&str]) -> SuperArgs {
        let mut argv = vec!["prodex", "s"];
        argv.extend(values.iter().copied());
        let crate::Commands::Super(mut args) =
            crate::parse_cli_command_from(argv).expect("Super command should parse")
        else {
            panic!("expected Super command");
        };
        args.extract_super_overrides_from_codex_args()
            .expect("Super tail should extract");
        *args
    }

    #[test]
    fn expose_freezes_prompted_main_and_sub_agent_choices() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(crate::SUB_AGENT_RECURSION_MARKER);
        let calls = RefCell::new(Vec::new());
        let mut args = super_args(&[]);

        resolve_super_expose_launch_decisions_with_prompts(
            &mut args,
            true,
            || {
                calls.borrow_mut().push("presidio");
                Ok(false)
            },
            |_| Ok(None),
            |_, locked| {
                assert_eq!(locked, None);
                calls.borrow_mut().push("main");
                Ok(ResolvedMainAgentConfig {
                    provider: ProviderId::Gemini,
                    model: Some("gemini-3.1-pro-preview".to_string()),
                    reasoning_effort: Some("high".to_string()),
                    local_url: None,
                })
            },
            |_| {
                calls.borrow_mut().push("sub-agent");
                Ok(Some(SubAgentConfig {
                    provider: ProviderId::Kiro,
                    model: Some("custom-sub-model".to_string()),
                    model_reasoning_effort: None,
                    url: None,
                    max_concurrency: Default::default(),
                }))
            },
        )
        .expect("expose choices should resolve");

        assert_eq!(&*calls.borrow(), &["main", "presidio", "sub-agent"]);
        assert_eq!(
            args.provider,
            Some(prodex_cli::SuperExternalProvider::Gemini)
        );
        assert_eq!(args.local_model.as_deref(), Some("gemini-3.1-pro-preview"));
        assert_eq!(
            codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort").as_deref(),
            Some("high")
        );
        assert!(!args.presidio);
        assert!(args.no_presidio);
        assert!(args.sub_agent);
        assert!(!args.no_sub_agent);
        assert_eq!(args.sub_agent_provider, Some(ProviderId::Kiro));
        assert_eq!(args.sub_agent_model.as_deref(), Some("custom-sub-model"));
    }

    #[test]
    fn expose_prompt_flow_does_not_depend_on_mode_or_expose_transport_flags() {
        let _marker =
            crate::test_support::TestEnvVarGuard::unset(crate::SUB_AGENT_RECURSION_MARKER);
        for argv in [
            vec!["prodex", "s", "expose"],
            vec!["prodex", "s", "expose", "exec"],
            vec!["prodex", "s", "expose", "--listen", "127.0.0.1:4567"],
            vec![
                "prodex",
                "s",
                "expose",
                "exec",
                "--openai-tunnel-id",
                "tunnel_0123456789abcdef0123456789abcdef",
            ],
        ] {
            let crate::Commands::SuperExpose(expose) =
                crate::parse_cli_command_from(argv).expect("expose command should parse")
            else {
                panic!("expected SuperExpose");
            };
            let mut args = expose.super_args.clone();
            let calls = RefCell::new(Vec::new());

            resolve_super_expose_launch_decisions_with_prompts(
                &mut args,
                true,
                || {
                    calls.borrow_mut().push("presidio");
                    Ok(false)
                },
                |_| Ok(None),
                |_, _| {
                    calls.borrow_mut().push("main");
                    Ok(ResolvedMainAgentConfig::without_effort(
                        ProviderId::OpenAi,
                        Some("gpt-5.6-sol".to_string()),
                        None,
                    ))
                },
                |_| {
                    calls.borrow_mut().push("sub-agent");
                    Ok(None)
                },
            )
            .expect("expose prompt flow should resolve");

            assert_eq!(&*calls.borrow(), &["main", "presidio", "sub-agent"]);
            assert_eq!(args.local_model.as_deref(), Some("gpt-5.6-sol"));
            assert!(args.no_sub_agent);
        }
    }

    #[test]
    fn expose_materialized_sub_agent_effort_is_preserved() {
        let mut args = super_args(&[]);
        let sub_agent = ResolvedSuperSubAgent {
            provider: ProviderId::OpenAi,
            model: Some("gpt-5.6-sol".to_string()),
            effort: Some(SubAgentReasoningEffort::Max),
            url: None,
            max_concurrency: Default::default(),
            target: crate::SubAgentLaunchTarget::Exec,
            presidio_enabled: false,
            required_tools: Vec::new(),
            recursion_disabled: true,
        };
        materialize_super_expose_launch_decisions(
            &mut args,
            true,
            &ResolvedMainAgentConfig::without_effort(
                ProviderId::OpenAi,
                Some("gpt-5.6-sol".to_string()),
                None,
            ),
            Some(&sub_agent),
        );

        assert_eq!(
            args.sub_agent_model_reasoning_effort,
            Some(SubAgentReasoningEffort::Max)
        );
        assert!(args.presidio);
        assert!(!args.no_presidio);
    }
}
