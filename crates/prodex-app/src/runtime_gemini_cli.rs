use crate::{
    PROVIDER_SECRET_ENV_KEYS, PreparedRuntimeLaunch, ResolvedSuperSubAgent, RuntimeLaunchRequest,
    RuntimeLaunchStrategy, RuntimeProxyEndpoint, agy_bin, clear_rtk_auto_wrap_control_env,
    copilot_bin, ensure_kiro_codebase_memory_compatibility,
    ensure_presidio_services_for_super_launch, ensure_required_presidio_services_for_super_launch,
    execute_runtime_launch, gemini_bin, kiro_bin, kiro_cli_data_dir_env, prepare_kiro_cli_data_dir,
    redact_super_session_args, render_sub_agent_disabled_dry_run_report,
    render_sub_agent_dry_run_report, resolve_super_launch_target,
};
use anyhow::{Context, Result, bail};
use prodex_cli::{
    SUPER_COPILOT_DEFAULT_MODEL, SUPER_COPILOT_PROVIDER_ID, SuperArgs, SuperCliAgent,
    SuperExternalProvider,
};
use prodex_runtime_launch::{ChildProcessPlan, RuntimeLaunchPlan, local_proxy_bypass_env};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

const PRODEX_COPILOT_PROXY_API_KEY: &str = "prodex-runtime-provider";
const NATIVE_GEMINI_AUTH_ENV_KEYS: [&str; 4] = [
    "GEMINI_API_KEYS",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEYS",
    "GOOGLE_API_KEY",
];
const KIRO_REMOVED_ENV_KEYS: [&str; 18] = [
    "ALL_PROXY",
    "AWS_DEFAULT_REGION",
    "AWS_ENDPOINT_URL",
    "AWS_REGION",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "KIRO_API_KEY",
    "KIRO_TEST_DB_PATH",
    "NO_PROXY",
    "PROXY",
    "Q_API_KEY",
    "all_proxy",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "proxy",
    "Q_ENDPOINT_URL",
    "KIRO_ENDPOINT_URL",
];
const KIRO_ROOT_SUBCOMMANDS: [&str; 12] = [
    "agent",
    "chat",
    "diagnostic",
    "issue",
    "login",
    "logout",
    "mcp",
    "profile",
    "serve",
    "settings",
    "update",
    "whoami",
];

struct SuperNativeCliLaunchStrategy {
    args: SuperArgs,
    presidio_enabled: bool,
    agent: SuperCliAgent,
    sub_agent: Option<ResolvedSuperSubAgent>,
}

impl RuntimeLaunchStrategy for SuperNativeCliLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate && self.agent == SuperCliAgent::Copilot,
            auto_redeem: false,
            skip_quota_check: true,
            base_url: self.args.base_url.as_deref().or(match self.agent {
                SuperCliAgent::Copilot => Some(SuperExternalProvider::Copilot.default_base_url()),
                _ => None,
            }),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: false,
            smart_context_enabled: self.agent == SuperCliAgent::Copilot,
            presidio_redaction_enabled: self.presidio_enabled
                && self.agent == SuperCliAgent::Copilot,
            model_context_window_tokens: self.args.local_context_window.map(|value| value as u64),
            gemini_thinking_budget_tokens: None,
            force_runtime_proxy: false,
            model_provider_override: match self.agent {
                SuperCliAgent::Copilot => Some(SUPER_COPILOT_PROVIDER_ID),
                _ => None,
            },
            profile_v2_name: None,
            external_provider: match self.agent {
                SuperCliAgent::Gemini => Some("gemini-native"),
                SuperCliAgent::Copilot => Some("copilot"),
                SuperCliAgent::Kiro => Some("kiro"),
                SuperCliAgent::Agy => Some("antigravity"),
                SuperCliAgent::Codex => None,
            },
            external_provider_api_key: (self.agent == SuperCliAgent::Copilot)
                .then_some(self.args.api_key.as_deref())
                .flatten(),
        }
    }

    fn build_plan(
        &mut self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        validate_super_native_cli_capabilities(&self.args, self.agent)?;
        let presidio_enabled = self.presidio_enabled && self.agent == SuperCliAgent::Copilot;
        let required_presidio = self
            .args
            .required_tools
            .contains(&prodex_optional_tools::OptionalToolId::Presidio);
        if required_presidio {
            ensure_required_presidio_services_for_super_launch(&prepared.paths)?;
        } else if presidio_enabled {
            ensure_presidio_services_for_super_launch(&prepared.paths)?;
        }
        let launch_args = runtime_super_native_cli_launch_args(
            self.agent,
            &self.args.codex_args,
            self.args.local_model.as_deref(),
        );
        if self.agent == SuperCliAgent::Agy {
            if self.sub_agent.is_some() {
                bail!("--sub-agent is unsupported for native Antigravity");
            }
            let mut child = ChildProcessPlan::new(agy_bin(), prepared.codex_home.clone())
                .with_args(launch_args);
            clear_rtk_auto_wrap_control_env(&mut child);
            return Ok(RuntimeLaunchPlan::new(child));
        }

        if self.sub_agent.is_some() {
            bail!(
                "--sub-agent is unsupported for native {agent:?} CLI launches",
                agent = self.agent
            );
        }
        let mut child = build_super_native_child(
            self.agent,
            &self.args,
            runtime_proxy,
            &prepared.codex_home,
            &prepared.codex_home,
            launch_args,
        )?;
        if self.agent == SuperCliAgent::Copilot {
            crate::remove_provider_secret_env(&mut child);
        }
        clear_rtk_auto_wrap_control_env(&mut child);
        if presidio_enabled {
            child.extra_env.push((
                OsString::from("PRODEX_PRESIDIO_ENABLED"),
                OsString::from("1"),
            ));
        }
        Ok(RuntimeLaunchPlan::new(child))
    }
}

fn build_super_native_child(
    agent: SuperCliAgent,
    args: &SuperArgs,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    codex_home: &Path,
    child_home: &Path,
    launch_args: Vec<OsString>,
) -> Result<ChildProcessPlan> {
    match agent {
        SuperCliAgent::Gemini => build_super_gemini_child(args, child_home, launch_args),
        SuperCliAgent::Copilot => {
            build_super_copilot_child(args, runtime_proxy, child_home, launch_args)
        }
        SuperCliAgent::Kiro => {
            build_super_kiro_child(runtime_proxy, codex_home, child_home, launch_args)
        }
        SuperCliAgent::Agy => unreachable!("Antigravity launch returns before overlay setup"),
        SuperCliAgent::Codex => bail!("Codex is not a native external CLI launch target"),
    }
}

fn build_super_gemini_child(
    args: &SuperArgs,
    child_home: &Path,
    launch_args: Vec<OsString>,
) -> Result<ChildProcessPlan> {
    let explicit_api_key = args.api_key.as_deref();
    if explicit_api_key.is_some_and(|value| value.trim().is_empty()) {
        bail!("--api-key must be non-empty");
    }

    let mut removed_env = BTreeSet::new();
    for key in PROVIDER_SECRET_ENV_KEYS {
        if explicit_api_key.is_some() || !NATIVE_GEMINI_AUTH_ENV_KEYS.contains(&key) {
            removed_env.insert(OsString::from(key));
        }
    }
    let mut child = ChildProcessPlan::new(gemini_bin(), child_home.to_path_buf())
        .with_args(launch_args)
        .with_removed_env(removed_env);
    if let Some(api_key) = explicit_api_key {
        child
            .extra_env
            .push((OsString::from("GEMINI_API_KEY"), OsString::from(api_key)));
    }
    Ok(child)
}

fn build_super_copilot_child(
    args: &SuperArgs,
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    child_home: &Path,
    launch_args: Vec<OsString>,
) -> Result<ChildProcessPlan> {
    let runtime_proxy =
        runtime_proxy.context("Copilot CLI launch requires a local runtime proxy")?;
    let model = args
        .local_model
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(SUPER_COPILOT_DEFAULT_MODEL);
    Ok(
        ChildProcessPlan::new(copilot_bin(), child_home.to_path_buf())
            .with_args(launch_args)
            .with_extra_env(runtime_super_copilot_cli_env(runtime_proxy, model))
            .with_removed_env([
                "COPILOT_PROVIDER_BEARER_TOKEN",
                "COPILOT_PROVIDER_MODEL_ID",
                "COPILOT_PROVIDER_WIRE_MODEL",
            ]),
    )
}

fn build_super_kiro_child(
    runtime_proxy: Option<&RuntimeProxyEndpoint>,
    codex_home: &Path,
    child_home: &Path,
    launch_args: Vec<OsString>,
) -> Result<ChildProcessPlan> {
    let runtime_proxy =
        runtime_proxy.context("native Kiro CLI launch requires a local transport tunnel")?;
    let proxy_url = runtime_proxy
        .kiro_connect_proxy_url()
        .context("native Kiro transport tunnel is unavailable")?;
    Ok(ChildProcessPlan::new(kiro_bin(), child_home.to_path_buf())
        .with_args(launch_args)
        .with_extra_env(runtime_super_kiro_cli_profile_env(codex_home, proxy_url)?)
        .with_removed_env(KIRO_REMOVED_ENV_KEYS))
}

fn runtime_super_copilot_cli_env(
    runtime_proxy: &RuntimeProxyEndpoint,
    model: &str,
) -> Vec<(OsString, OsString)> {
    let proxy_base_url = format!(
        "http://{}{}",
        runtime_proxy.listen_addr, runtime_proxy.openai_mount_path
    );
    let mut env = vec![
        (
            OsString::from("COPILOT_PROVIDER_BASE_URL"),
            OsString::from(proxy_base_url),
        ),
        (
            OsString::from("COPILOT_PROVIDER_TYPE"),
            OsString::from("openai"),
        ),
        (
            OsString::from("COPILOT_PROVIDER_API_KEY"),
            OsString::from(PRODEX_COPILOT_PROXY_API_KEY),
        ),
        (
            OsString::from("COPILOT_PROVIDER_WIRE_API"),
            OsString::from("responses"),
        ),
        (
            OsString::from("COPILOT_PROVIDER_TRANSPORT"),
            OsString::from("http"),
        ),
        (OsString::from("COPILOT_MODEL"), OsString::from(model)),
    ];
    env.extend(
        local_proxy_bypass_env()
            .into_iter()
            .map(|(key, value)| (OsString::from(key), value)),
    );
    env
}

fn runtime_super_native_cli_launch_args(
    agent: SuperCliAgent,
    args: &[OsString],
    model: Option<&str>,
) -> Vec<OsString> {
    if agent == SuperCliAgent::Kiro {
        return runtime_super_kiro_cli_launch_args(args, model);
    }
    let mut launch_args = args.to_vec();
    match agent {
        SuperCliAgent::Gemini
            if !launch_args.iter().any(|arg| {
                arg.to_str().is_some_and(|arg| {
                    matches!(arg, "--yolo" | "-y" | "--approval-mode")
                        || arg.starts_with("--yolo=")
                        || arg.starts_with("--approval-mode=")
                })
            }) =>
        {
            launch_args.insert(0, OsString::from("--yolo"));
        }
        SuperCliAgent::Agy
            if !launch_args
                .iter()
                .any(|arg| arg == "--dangerously-skip-permissions") =>
        {
            launch_args.insert(0, OsString::from("--dangerously-skip-permissions"));
        }
        _ => {}
    }
    if let Some(model) = model
        && !launch_args.iter().any(runtime_cli_arg_sets_model)
    {
        launch_args.splice(0..0, [OsString::from("--model"), OsString::from(model)]);
    }
    launch_args
}

fn runtime_super_kiro_cli_launch_args(args: &[OsString], model: Option<&str>) -> Vec<OsString> {
    let normalized = prodex_runtime_launch::normalize_run_codex_args(args);
    if let Some(session_id) = prodex_runtime_launch::codex_resume_session_id(&normalized)
        && let Some(target_index) = normalized
            .iter()
            .position(|arg| arg.to_str() == Some(session_id))
    {
        let mut launch_args = vec![
            OsString::from("chat"),
            OsString::from("--resume-id"),
            OsString::from(session_id),
        ];
        if let Some(model) = model
            && !normalized[target_index + 1..]
                .iter()
                .any(runtime_cli_arg_sets_model)
        {
            launch_args.extend([OsString::from("--model"), OsString::from(model)]);
        }
        launch_args.extend(normalized[target_index + 1..].iter().cloned());
        return launch_args;
    }
    if model.is_none() || args.iter().any(runtime_cli_arg_sets_model) {
        return args.to_vec();
    }
    if args.iter().any(|arg| {
        arg.to_str().is_some_and(|arg| {
            matches!(arg, "--help" | "-h" | "--version" | "-V" | "help")
                || KIRO_ROOT_SUBCOMMANDS
                    .iter()
                    .any(|command| arg == *command && arg != "chat")
        })
    }) {
        return args.to_vec();
    }
    if let Some(chat_index) = args.iter().position(|arg| arg == "chat") {
        let mut launch_args = args.to_vec();
        launch_args.splice(
            chat_index + 1..chat_index + 1,
            [OsString::from("--model"), OsString::from(model.unwrap())],
        );
        return launch_args;
    }
    let mut launch_args = Vec::with_capacity(args.len() + 3);
    launch_args.push(OsString::from("chat"));
    launch_args.push(OsString::from("--model"));
    launch_args.push(OsString::from(model.unwrap_or_default()));
    launch_args.extend(args.iter().cloned());
    launch_args
}

fn runtime_cli_arg_sets_model(arg: &OsString) -> bool {
    arg.to_str().is_some_and(|arg| {
        matches!(arg, "--model" | "-m") || arg.starts_with("--model=") || arg.starts_with("-m=")
    })
}

fn runtime_super_kiro_cli_profile_env(
    codex_home: &Path,
    proxy_url: &str,
) -> Result<Vec<(OsString, OsString)>> {
    ensure_kiro_codebase_memory_compatibility()
        .context("failed to prepare Kiro Codebase Memory MCP compatibility")?;
    let (data_dir, secret) = prepare_kiro_cli_data_dir(codex_home)?;
    let mut env = kiro_cli_data_dir_env(&data_dir);
    if let Some(region) = secret.region.filter(|value| !value.trim().is_empty()) {
        env.push((OsString::from("AWS_REGION"), OsString::from(region)));
    }
    env.extend([
        (
            OsString::from("AWS_IGNORE_CONFIGURED_ENDPOINT_URLS"),
            OsString::from("true"),
        ),
        (OsString::from("KIRO_NO_AUTO_UPDATE"), OsString::from("1")),
        (OsString::from("HTTP_PROXY"), OsString::from(proxy_url)),
        (OsString::from("HTTPS_PROXY"), OsString::from(proxy_url)),
        (OsString::from("http_proxy"), OsString::from(proxy_url)),
        (OsString::from("https_proxy"), OsString::from(proxy_url)),
        (OsString::from("NO_PROXY"), OsString::new()),
        (OsString::from("no_proxy"), OsString::new()),
    ]);
    Ok(env)
}

pub(super) fn handle_super_native_cli(
    args: SuperArgs,
    presidio_enabled: bool,
    sub_agent: Option<ResolvedSuperSubAgent>,
) -> Result<()> {
    let agent = validate_super_native_cli_args(&args)?;
    if sub_agent.is_some() && agent != SuperCliAgent::Codex {
        bail!("--sub-agent is supported only on the Codex Super bridge, not native CLI launches");
    }
    execute_runtime_launch(SuperNativeCliLaunchStrategy {
        args,
        presidio_enabled,
        agent,
        sub_agent,
    })
}

pub(super) fn handle_super_native_cli_dry_run(
    args: SuperArgs,
    sub_agent: Option<&ResolvedSuperSubAgent>,
) -> Result<()> {
    let report = super_native_cli_dry_run_report(&args, sub_agent)?;
    crate::print_runtime_launch_dry_run_report("native-cli", &report)
}

pub(super) fn validate_super_native_cli_preflight(args: &SuperArgs) -> Result<()> {
    let mut effective = args.clone();
    effective
        .extract_super_overrides_from_codex_args_for_native_preflight()
        .map_err(anyhow::Error::msg)?;
    let Some(agent) = effective.cli else {
        effective.validate_urls().map_err(anyhow::Error::msg)?;
        return Ok(());
    };
    if agent == SuperCliAgent::Codex {
        effective.validate_urls().map_err(anyhow::Error::msg)?;
        return Ok(());
    }
    validate_super_native_cli_capability_args(&effective)?;
    effective.validate_urls().map_err(anyhow::Error::msg)?;
    validate_super_native_cli_args(&effective)?;
    if agent != SuperCliAgent::Agy {
        return Ok(());
    }
    let mut command = Command::new(agy_bin());
    command.arg("--version").env_remove("CONTROL_PLANE_API_KEY");
    match crate::command_probe_output(&mut command, "Antigravity CLI version probe") {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) => bail!("native Antigravity CLI capability `agy` is unavailable"),
        Err(_) => bail!("native Antigravity CLI capability `agy` is unavailable"),
    }
}

pub(super) fn validate_super_native_cli_capability_args(args: &SuperArgs) -> Result<()> {
    let Some(agent) = args.cli.filter(|agent| *agent != SuperCliAgent::Codex) else {
        return Ok(());
    };
    validate_super_native_cli_capabilities(args, agent)
}

fn validate_super_native_cli_args(args: &SuperArgs) -> Result<SuperCliAgent> {
    let agent = args.cli.context("native external agent CLI is missing")?;
    validate_super_native_cli_capabilities(args, agent)?;
    if agent == SuperCliAgent::Gemini && args.profile.is_some() {
        bail!(
            "--profile is unsupported by the native Gemini CLI path; use authentication owned by Gemini CLI or its environment"
        );
    }
    match agent {
        SuperCliAgent::Gemini | SuperCliAgent::Agy
            if args.provider != Some(SuperExternalProvider::Gemini) =>
        {
            bail!(
                "--provider is incompatible with the selected native {agent:?} frontend; use `--provider gemini`"
            )
        }
        SuperCliAgent::Copilot if args.provider != Some(SuperExternalProvider::Copilot) => {
            bail!(
                "--provider is incompatible with the selected native Copilot frontend; use `--provider copilot`"
            )
        }
        SuperCliAgent::Kiro if args.provider.is_some() => {
            bail!(
                "--provider is unsupported by the selected native Kiro frontend; native Kiro uses imported profiles directly"
            )
        }
        _ => {}
    }
    Ok(agent)
}

fn validate_super_native_cli_capabilities(args: &SuperArgs, agent: SuperCliAgent) -> Result<()> {
    if agent == SuperCliAgent::Codex {
        return Ok(());
    }

    let frontend = match agent {
        SuperCliAgent::Gemini => "Gemini",
        SuperCliAgent::Copilot => "Copilot",
        SuperCliAgent::Kiro => "Kiro",
        SuperCliAgent::Agy => "Antigravity (agy)",
        SuperCliAgent::Codex => unreachable!(),
    };
    let unsupported = |option: &str| -> anyhow::Error {
        anyhow::anyhow!("{option} is unsupported by the selected native {frontend} frontend")
    };

    if args.harness.is_some() {
        return Err(unsupported("--harness"));
    }
    if args.api_key.is_some() && !matches!(agent, SuperCliAgent::Gemini | SuperCliAgent::Copilot) {
        return Err(unsupported(concat!("--api", "-key")));
    }
    if let Some(option) = first_unsupported_native_option(args) {
        return Err(unsupported(option));
    }
    if args.sub_agent {
        return Err(anyhow::anyhow!(
            "--sub-agent is unsupported by the selected native {frontend} frontend; it is supported only on the Codex Super bridge"
        ));
    }

    if let Some(option) = first_unsupported_native_tool_option(args, agent) {
        return Err(unsupported(&option));
    }
    if args.presidio
        && matches!(
            agent,
            SuperCliAgent::Gemini | SuperCliAgent::Kiro | SuperCliAgent::Agy
        )
    {
        return Err(unsupported("--presidio"));
    }

    if agent == SuperCliAgent::Agy
        && let Some(option) = first_unsupported_agy_option(args)
    {
        return Err(unsupported(option));
    }
    Ok(())
}

fn first_unsupported_native_tool_option(args: &SuperArgs, agent: SuperCliAgent) -> Option<String> {
    let copilot_presidio = agent == SuperCliAgent::Copilot;
    args.required_tools
        .iter()
        .find_map(|tool| {
            (!copilot_presidio || *tool != prodex_optional_tools::OptionalToolId::Presidio)
                .then(|| format!("--require-tool {tool}"))
        })
        .or_else(|| {
            args.tools.iter().find_map(|tool| {
                (!copilot_presidio || *tool != prodex_optional_tools::OptionalToolId::Presidio)
                    .then(|| format!("--tool {tool}"))
            })
        })
}

fn first_unsupported_native_option(args: &SuperArgs) -> Option<&'static str> {
    let feature = &args.codex_features;
    [
        (feature.web_search.is_some(), "--web-search"),
        (
            feature.rollout_budget_tokens.is_some(),
            "--rollout-budget-tokens",
        ),
        (
            !feature.rollout_budget_reminders.is_empty(),
            "--rollout-budget-reminders",
        ),
        (
            feature.rollout_budget_sampling_weight.is_some(),
            "--rollout-budget-sampling-weight",
        ),
        (
            feature.rollout_budget_prefill_weight.is_some(),
            "--rollout-budget-prefill-weight",
        ),
        (feature.current_time_reminder, "--current-time-reminder"),
        (
            feature.current_time_reminder_interval.is_some(),
            "--current-time-reminder-interval",
        ),
        (
            feature.current_time_clock_source.is_some(),
            "--current-time-clock-source",
        ),
        (feature.respect_system_proxy, "--respect-system-proxy"),
        (feature.no_respect_system_proxy, "--no-respect-system-proxy"),
        (args.sub_agent_provider.is_some(), "--sub-agent-provider"),
        (args.sub_agent_model.is_some(), "--sub-agent-model"),
        (
            args.sub_agent_model_reasoning_effort.is_some(),
            "--sub-agent-model-reasoning-effort",
        ),
        (args.sub_agent_url.is_some(), "--sub-agent-url"),
    ]
    .into_iter()
    .find_map(|(configured, option)| configured.then_some(option))
}

fn first_unsupported_agy_option(args: &SuperArgs) -> Option<&'static str> {
    [
        (args.auto_rotate, "--auto-rotate"),
        (args.auto_redeem, "--auto-redeem"),
        (args.skip_quota_check, "--skip-quota-check"),
        (args.base_url.is_some(), "--base-url"),
        (args.no_proxy, "--no-proxy"),
        (args.url.is_some(), "--url"),
        (args.local_context_window.is_some(), "--context-window"),
        (
            args.local_auto_compact_token_limit.is_some(),
            "--auto-compact-token-limit",
        ),
        (
            matches!(
                resolve_super_launch_target(&args.codex_args),
                prodex_cli::SuperLaunchTarget::Resume { .. }
            ),
            "Codex session resume",
        ),
    ]
    .into_iter()
    .find_map(|(configured, option)| configured.then_some(option))
}

fn super_native_cli_dry_run_report(
    args: &SuperArgs,
    sub_agent: Option<&ResolvedSuperSubAgent>,
) -> Result<String> {
    let agent = validate_super_native_cli_args(args)?;
    let binary = match agent {
        SuperCliAgent::Gemini => gemini_bin(),
        SuperCliAgent::Copilot => copilot_bin(),
        SuperCliAgent::Kiro => kiro_bin(),
        SuperCliAgent::Agy => agy_bin(),
        SuperCliAgent::Codex => bail!("Codex is not a native external CLI launch target"),
    };
    let provider = match agent {
        SuperCliAgent::Gemini => "gemini",
        SuperCliAgent::Copilot => "copilot",
        SuperCliAgent::Kiro => "kiro",
        SuperCliAgent::Agy => "antigravity",
        SuperCliAgent::Codex => unreachable!(),
    };
    let model = args
        .local_model
        .as_deref()
        .or((agent == SuperCliAgent::Copilot).then_some(SUPER_COPILOT_DEFAULT_MODEL))
        .unwrap_or("(CLI default)");
    let model = redaction::redaction_redact_secret_like_text(model);
    let proxy = match agent {
        SuperCliAgent::Gemini => "disabled; native Gemini CLI owns transport and authentication",
        SuperCliAgent::Copilot => "would use local provider bridge",
        SuperCliAgent::Kiro => "would use authenticated CONNECT tunnel",
        SuperCliAgent::Agy => "disabled",
        SuperCliAgent::Codex => unreachable!(),
    };
    let launch_args =
        runtime_super_native_cli_launch_args(agent, &args.codex_args, args.local_model.as_deref());
    let launch_args = redact_super_session_args(&launch_args);
    let binary_name = std::path::Path::new(&binary)
        .file_name()
        .unwrap_or(binary.as_os_str());
    let mut output = format!(
        "Prodex dry run: launch diagnostics\nFlow: native-cli\nBinary: {}\nProvider: {provider}\nModel: {model}\nProfile: {}\nRuntime proxy: {proxy}\nArgs:\n",
        redaction::redaction_display_os(binary_name),
        if agent == SuperCliAgent::Gemini {
            "(native CLI owned)"
        } else if args.profile.is_some() {
            "<configured>"
        } else {
            "(active/default)"
        }
    );
    if launch_args.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for arg in redaction::redaction_redacted_cli_args(&launch_args) {
            output.push_str(&format!("  {arg}\n"));
        }
    }
    output.push_str(
        "Credentials: resolved only at launch\nNative CLI not started because --dry-run was set.\n",
    );
    if let Some(sub_agent) = sub_agent {
        output.push_str(&render_sub_agent_dry_run_report(sub_agent));
    } else {
        output.push_str(&render_sub_agent_disabled_dry_run_report(
            args.presidio && agent == SuperCliAgent::Copilot,
        ));
    }
    Ok(output)
}

#[cfg(test)]
#[path = "runtime_gemini_cli/tests/cases.rs"]
mod tests;
