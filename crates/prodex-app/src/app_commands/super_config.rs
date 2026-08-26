#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedMainAgentConfig {
    pub(crate) provider: prodex_provider_core::ProviderId,
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) local_url: Option<String>,
}

impl ResolvedMainAgentConfig {
    pub(super) fn without_effort(
        provider: prodex_provider_core::ProviderId,
        model: Option<String>,
        local_url: Option<String>,
    ) -> Self {
        Self {
            provider,
            model,
            reasoning_effort: None,
            local_url,
        }
    }
}

use crate::{codex_cli_config_override_value, provider_display_name};
use anyhow::{Result, bail};
use prodex_cli::{SuperArgs, SuperCliAgent, SuperExternalProvider};
use std::ffi::OsString;

pub(super) fn resolve_main_agent_config(
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
            Ok(ResolvedMainAgentConfig::without_effort(
                provider,
                args.local_model.clone(),
                args.url.clone(),
            ))
        }
    } else if let Some(url) = args.url.clone() {
        Ok(ResolvedMainAgentConfig::without_effort(
            prodex_provider_core::ProviderId::Local,
            args.local_model.clone(),
            Some(url),
        ))
    } else if let Some(provider) = args.provider {
        Ok(ResolvedMainAgentConfig::without_effort(
            provider.provider_id(),
            args.local_model.clone(),
            None,
        ))
    } else if let Some(provider) = explicit_provider {
        Ok(ResolvedMainAgentConfig::without_effort(
            provider,
            args.local_model.clone(),
            None,
        ))
    } else if interactive {
        prompt(args, None)
    } else {
        Ok(ResolvedMainAgentConfig::without_effort(
            prodex_provider_core::ProviderId::OpenAi,
            args.local_model.clone(),
            None,
        ))
    }
}

pub(super) fn apply_resolved_main_agent(
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

pub(super) fn resolve_super_main_agent_with_prompt(
    args: &mut SuperArgs,
    interactive: bool,
    session_provider: Option<prodex_provider_core::ProviderId>,
    prompt: impl FnOnce(
        &SuperArgs,
        Option<prodex_provider_core::ProviderId>,
    ) -> Result<ResolvedMainAgentConfig>,
    order: super::SuperPromptOrder,
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

    let resolved = if order == super::SuperPromptOrder::MainAgentFirst
        && interactive
        && session_provider.is_none()
        && explicit_provider.is_some()
    {
        let Some(expected_provider) = explicit_provider else {
            return Err(anyhow::anyhow!("main-agent provider resolution failed"));
        };
        let displayed = prompt(args, Some(expected_provider))?;
        if displayed.provider != expected_provider {
            bail!("explicit main-agent provider display returned a conflicting provider");
        }
        displayed
    } else {
        resolve_main_agent_config(
            args,
            interactive,
            session_provider,
            explicit_provider,
            prompt,
        )?
    };
    if order == super::SuperPromptOrder::MainAgentFirst && !interactive {
        let (model, reasoning_effort) = super::super_main_prompt::resolve_main_model_and_effort(
            args,
            resolved.provider,
            false,
        )?;
        let resolved = ResolvedMainAgentConfig {
            model,
            reasoning_effort,
            ..resolved
        };
        apply_resolved_main_agent(args, &resolved)?;
        return Ok(resolved);
    }
    apply_resolved_main_agent(args, &resolved)?;
    Ok(resolved)
}
