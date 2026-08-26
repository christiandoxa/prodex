use super::{
    ResolvedMainAgentConfig, ResolvedSuperSubAgent, SuperArgs, SuperPromptOrder,
    resolve_super_launch_decisions_with_order, super_prompt,
};
use crate::codex_cli_config_override_value;
use anyhow::Result;
use std::ffi::OsString;

pub(crate) fn resolve_super_expose_configuration(
    args: &mut SuperArgs,
    interactive: bool,
) -> Result<(bool, ResolvedMainAgentConfig, Option<ResolvedSuperSubAgent>)> {
    let session_provider =
        super::runtime_launch::runtime_resume_provider_from_codex_args(&args.codex_args)?;
    let decisions = resolve_super_launch_decisions_with_order(
        args,
        interactive,
        SuperPromptOrder::MainAgentFirst,
        super_prompt::prompt_super_presidio_opt_in,
        |_| Ok(session_provider),
        super_prompt::prompt_super_main_agent_configuration_for_expose,
        super_prompt::prompt_super_sub_agent_configuration,
    )?;
    apply_main_configuration(args, &decisions.1)?;
    args.presidio = decisions.0;
    args.no_presidio = !decisions.0;
    if let Some(sub_agent) = decisions.2.as_ref() {
        args.sub_agent = true;
        args.no_sub_agent = false;
        args.sub_agent_provider = Some(sub_agent.provider);
        args.sub_agent_model = sub_agent.model.clone();
        args.sub_agent_model_reasoning_effort = sub_agent.effort;
        args.sub_agent_url = sub_agent.url.clone();
        args.sub_agent_max_concurrency = Some(sub_agent.max_concurrency);
    } else {
        args.sub_agent = false;
        args.no_sub_agent = true;
        args.sub_agent_provider = None;
        args.sub_agent_model = None;
        args.sub_agent_model_reasoning_effort = None;
        args.sub_agent_url = None;
        args.sub_agent_max_concurrency = None;
    }
    Ok(decisions)
}

fn apply_main_configuration(
    args: &mut SuperArgs,
    resolved: &ResolvedMainAgentConfig,
) -> Result<()> {
    super::super_config::apply_resolved_main_agent(args, resolved)?;
    if args.local_model.is_none() {
        args.local_model = resolved.model.clone();
    }
    if let Some(effort) = resolved.reasoning_effort.as_deref()
        && codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort").is_none()
    {
        args.codex_args.extend([
            OsString::from("-c"),
            OsString::from(format!(
                "model_reasoning_effort={}",
                crate::runtime_catalog_config::toml_string_literal(effort)
            )),
        ]);
    }
    Ok(())
}
