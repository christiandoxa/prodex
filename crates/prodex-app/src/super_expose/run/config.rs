use super::*;
use prodex_cli::SuperExternalProvider;
use std::ffi::OsString;

pub(super) fn main_provider(args: &SuperArgs) -> prodex_provider_core::ProviderId {
    args.url
        .as_ref()
        .map(|_| prodex_provider_core::ProviderId::Local)
        .or_else(|| args.provider.map(SuperExternalProvider::provider_id))
        .or_else(|| {
            crate::codex_cli_config_override_value(&args.codex_args, "model_provider").and_then(
                |provider| {
                    prodex_provider_core::provider_implementation_registry()
                        .resolve_model_provider_id(&provider)
                },
            )
        })
        .unwrap_or(prodex_provider_core::ProviderId::OpenAi)
}

pub(super) fn validate_run_configuration(args: &SuperArgs) -> std::result::Result<(), String> {
    if args.cli == Some(prodex_cli::SuperCliAgent::Agy) {
        crate::app_commands::validate_native_agy_args(args).map_err(|error| error.to_string())
    } else {
        args.validate_urls().map_err(|error| error.to_string())
    }
}

pub(super) fn apply_overrides(
    args: &mut SuperArgs,
    values: &Value,
) -> std::result::Result<(), String> {
    if let Some(provider) = optional_bounded_string(values, "provider", 256)? {
        apply_provider_override(args, &provider)?;
    }
    if let Some(model) = optional_bounded_string(values, "model", 256)? {
        args.local_model = Some(model);
    }
    if let Some(effort) = optional_bounded_string(values, "reasoning_effort", 256)? {
        let parsed = effort
            .parse::<prodex_cli::SubAgentReasoningEffort>()
            .map_err(|_| "reasoning_effort is unsupported".to_string())?;
        let provider = main_provider(args);
        let configured_model = crate::codex_cli_config_override_value(&args.codex_args, "model");
        let model = args.local_model.as_deref().or(configured_model.as_deref());
        if !crate::canonical_sub_agent_efforts(provider, model).contains(&parsed) {
            return Err("reasoning_effort is unsupported for the selected model".to_string());
        }
        remove_codex_config_override(&mut args.codex_args, "model_reasoning_effort");
        args.codex_args.extend([
            OsString::from("-c"),
            OsString::from(format!(
                "model_reasoning_effort={}",
                crate::runtime_catalog_config::toml_string_literal(&effort)
            )),
        ]);
    }
    if let Some(profile) = optional_bounded_string(values, "profile", 128)? {
        prodex_profile_identity::validate_profile_name(&profile)
            .map_err(|_| "profile is invalid".to_string())?;
        args.profile = Some(profile);
    }
    if let Some(sub_agents) = values.get("sub_agents")
        && !sub_agents.is_null()
    {
        let Some(sub_agents) = sub_agents.as_bool() else {
            return Err("sub_agents must be a boolean".to_string());
        };
        if sub_agents {
            args.sub_agent = true;
            args.no_sub_agent = false;
        } else {
            args.sub_agent = false;
            args.no_sub_agent = true;
            args.sub_agent_provider = None;
            args.sub_agent_model = None;
            args.sub_agent_model_reasoning_effort = None;
            args.sub_agent_url = None;
            args.sub_agent_max_concurrency = None;
        }
    }
    validate_configured_reasoning_effort(args)
}

fn apply_provider_override(
    args: &mut SuperArgs,
    provider: &str,
) -> std::result::Result<(), String> {
    let provider = prodex_provider_core::ProviderId::parse(provider)
        .ok_or_else(|| "provider is unsupported".to_string())?;
    let provider_changed = main_provider(args) != provider;
    if provider_changed {
        args.api_key = None;
        args.local_model = None;
        remove_codex_config_override(&mut args.codex_args, "model");
        remove_codex_config_override(&mut args.codex_args, "model_reasoning_effort");
    }
    remove_codex_config_override(&mut args.codex_args, "model_provider");
    match provider {
        prodex_provider_core::ProviderId::OpenAi => {
            args.provider = None;
            args.url = None;
            args.codex_args.extend([
                OsString::from("-c"),
                OsString::from("model_provider=\"openai\""),
            ]);
        }
        prodex_provider_core::ProviderId::Local => {
            if args.url.is_none() {
                return Err("local provider requires the expose local URL".to_string());
            }
            args.provider = None;
        }
        provider => {
            args.url = None;
            args.provider = SuperExternalProvider::from_provider_id(provider);
            if args.provider.is_none() {
                return Err("provider is unsupported".to_string());
            }
        }
    }
    Ok(())
}

fn validate_configured_reasoning_effort(args: &SuperArgs) -> std::result::Result<(), String> {
    let Some(effort) =
        crate::codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort")
    else {
        return Ok(());
    };
    let parsed = effort
        .parse::<prodex_cli::SubAgentReasoningEffort>()
        .map_err(|_| "reasoning_effort is unsupported".to_string())?;
    let configured_model = crate::codex_cli_config_override_value(&args.codex_args, "model");
    let model = args.local_model.as_deref().or(configured_model.as_deref());
    if crate::canonical_sub_agent_efforts(main_provider(args), model).contains(&parsed) {
        Ok(())
    } else {
        Err("reasoning_effort is unsupported for the selected model".to_string())
    }
}

fn remove_codex_config_override(args: &mut Vec<OsString>, key: &str) {
    let mut retained = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].to_string_lossy();
        if matches!(argument.as_ref(), "-c" | "--config")
            && args
                .get(index + 1)
                .and_then(|value| value.to_str())
                .is_some_and(|assignment| config_assignment_has_key(assignment, key))
        {
            index += 2;
            continue;
        }
        if (argument.starts_with("--config=") || argument.starts_with("-c"))
            && config_assignment_has_key(
                argument
                    .strip_prefix("--config=")
                    .or_else(|| argument.strip_prefix("-c"))
                    .unwrap_or_default(),
                key,
            )
        {
            index += 1;
            continue;
        }
        retained.push(args[index].clone());
        index += 1;
    }
    *args = retained;
}

fn config_assignment_has_key(assignment: &str, key: &str) -> bool {
    assignment
        .split_once('=')
        .is_some_and(|(name, _)| name.trim() == key)
}

fn optional_bounded_string(
    values: &Value,
    name: &str,
    max_bytes: usize,
) -> std::result::Result<Option<String>, String> {
    let Some(value) = values.get(name) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(value) = value.as_str() else {
        return Err(format!("{name} must be a string"));
    };
    if value.is_empty()
        || value.len() > max_bytes
        || value.as_bytes().contains(&0)
        || value.chars().any(char::is_control)
    {
        return Err(format!("{name} is empty or too large"));
    }
    Ok(Some(value.to_string()))
}
