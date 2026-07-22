use crate::{
    PreparedRuntimeLaunch, RuntimeLaunchRequest, RuntimeLaunchStrategy, RuntimeProxyEndpoint,
    agy_bin, clear_rtk_auto_wrap_control_env, copilot_bin, execute_runtime_launch, gemini_bin,
    kiro_bin, kiro_cli_data_dir_env, prepare_kiro_cli_data_dir, prepare_prodex_overlay_home,
    prepend_child_path, refresh_gemini_oauth_secret_if_needed,
};
use anyhow::{Context, Result, bail};
use prodex_cli::{
    SUPER_COPILOT_DEFAULT_MODEL, SUPER_COPILOT_PROVIDER_ID, SUPER_GEMINI_DEFAULT_BASE_URL,
    SUPER_GEMINI_PROVIDER_ID, SuperArgs, SuperCliAgent, SuperExternalProvider,
};
use prodex_runtime_launch::{ChildProcessPlan, RuntimeLaunchPlan, local_proxy_bypass_env};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

const PRODEX_COPILOT_PROXY_API_KEY: &str = "prodex-runtime-provider";
const GEMINI_SETTINGS_FILE_LIMIT: u64 = 512 * 1024;
const GEMINI_REMOVED_ENV_KEYS: [&str; 11] = [
    "GEMINI_CLI_SYSTEM_DEFAULTS_PATH",
    "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
    "GEMINI_CLI_USE_COMPUTE_ADC",
    "GEMINI_DEFAULT_AUTH_TYPE",
    "GOOGLE_CLOUD_ACCESS_TOKEN",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_PROJECT_ID",
    "GOOGLE_CLOUD_QUOTA_PROJECT",
    "GOOGLE_GEMINI_BASE_URL",
    "GOOGLE_GENAI_USE_GCA",
    "GOOGLE_GENAI_USE_VERTEXAI",
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
}

impl RuntimeLaunchStrategy for SuperNativeCliLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate
                && matches!(self.agent, SuperCliAgent::Gemini | SuperCliAgent::Copilot),
            auto_redeem: false,
            skip_quota_check: true,
            base_url: self.args.base_url.as_deref().or(match self.agent {
                SuperCliAgent::Gemini => Some(SUPER_GEMINI_DEFAULT_BASE_URL),
                SuperCliAgent::Copilot => Some(SuperExternalProvider::Copilot.default_base_url()),
                _ => None,
            }),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: false,
            smart_context_enabled: matches!(
                self.agent,
                SuperCliAgent::Gemini | SuperCliAgent::Copilot
            ),
            presidio_redaction_enabled: self.presidio_enabled
                && matches!(self.agent, SuperCliAgent::Gemini | SuperCliAgent::Copilot),
            model_context_window_tokens: self.args.local_context_window.map(|value| value as u64),
            gemini_thinking_budget_tokens: None,
            force_runtime_proxy: false,
            model_provider_override: match self.agent {
                SuperCliAgent::Gemini => Some(SUPER_GEMINI_PROVIDER_ID),
                SuperCliAgent::Copilot => Some(SUPER_COPILOT_PROVIDER_ID),
                _ => None,
            },
            profile_v2_name: None,
            external_provider: match self.agent {
                SuperCliAgent::Gemini => Some("gemini-oauth"),
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
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        let presidio_enabled = self.presidio_enabled
            && matches!(self.agent, SuperCliAgent::Gemini | SuperCliAgent::Copilot);
        if presidio_enabled {
            crate::ensure_presidio_services_for_super_launch(&prepared.paths)?;
        }
        let launch_args = runtime_super_native_cli_launch_args(
            self.agent,
            &self.args.codex_args,
            self.args.local_model.as_deref(),
        );
        if self.agent == SuperCliAgent::Agy {
            let mut child = ChildProcessPlan::new(agy_bin(), prepared.codex_home.clone())
                .with_args(launch_args);
            clear_rtk_auto_wrap_control_env(&mut child);
            return Ok(RuntimeLaunchPlan::new(child));
        }

        let overlay_home = prepare_prodex_overlay_home(&prepared.paths, &prepared.codex_home)?;
        prodex_caveman_assets::configure_rtk_codex_home(&overlay_home)?;
        prodex_caveman_assets::configure_super_optimizer_codex_home_with_presidio(
            &overlay_home,
            presidio_enabled,
        )?;
        let mut child = match self.agent {
            SuperCliAgent::Gemini => {
                let runtime_proxy =
                    runtime_proxy.context("Gemini CLI launch requires a local runtime proxy")?;
                let proxy_base_url = format!("http://{}", runtime_proxy.listen_addr);
                let mut gemini_auth_env = runtime_super_gemini_cli_oauth_env(&prepared.codex_home)?;
                let (system_settings, system_defaults) =
                    runtime_super_gemini_cli_system_settings(&overlay_home)?;
                gemini_auth_env.extend(
                    local_proxy_bypass_env()
                        .into_iter()
                        .map(|(key, value)| (OsString::from(key), value)),
                );
                ChildProcessPlan::new(gemini_bin(), prepared.codex_home.clone())
                    .with_args(launch_args)
                    .with_extra_env(gemini_auth_env.into_iter().chain([
                        (
                            OsString::from("GOOGLE_GENAI_USE_GCA"),
                            OsString::from("true"),
                        ),
                        (
                            OsString::from("CODE_ASSIST_ENDPOINT"),
                            OsString::from(proxy_base_url),
                        ),
                        (
                            OsString::from("CODE_ASSIST_API_VERSION"),
                            OsString::from("v1internal"),
                        ),
                        (
                            OsString::from("GEMINI_CLI_SYSTEM_SETTINGS_PATH"),
                            system_settings.into_os_string(),
                        ),
                        (
                            OsString::from("GEMINI_CLI_SYSTEM_DEFAULTS_PATH"),
                            system_defaults.into_os_string(),
                        ),
                    ]))
                    .with_removed_env(GEMINI_REMOVED_ENV_KEYS)
            }
            SuperCliAgent::Copilot => {
                let runtime_proxy =
                    runtime_proxy.context("Copilot CLI launch requires a local runtime proxy")?;
                let model = self
                    .args
                    .local_model
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(SUPER_COPILOT_DEFAULT_MODEL);
                ChildProcessPlan::new(copilot_bin(), prepared.codex_home.clone())
                    .with_args(launch_args)
                    .with_extra_env(runtime_super_copilot_cli_env(runtime_proxy, model))
                    .with_removed_env([
                        "COPILOT_PROVIDER_BEARER_TOKEN",
                        "COPILOT_PROVIDER_MODEL_ID",
                        "COPILOT_PROVIDER_WIRE_MODEL",
                    ])
            }
            SuperCliAgent::Agy => unreachable!("Antigravity launch returns before overlay setup"),
            SuperCliAgent::Kiro => {
                let runtime_proxy = runtime_proxy
                    .context("native Kiro CLI launch requires a local transport tunnel")?;
                let proxy_url = runtime_proxy
                    .kiro_connect_proxy_url()
                    .context("native Kiro transport tunnel is unavailable")?;
                ChildProcessPlan::new(kiro_bin(), prepared.codex_home.clone())
                    .with_args(launch_args)
                    .with_extra_env(runtime_super_kiro_cli_profile_env(
                        &prepared.codex_home,
                        proxy_url,
                    )?)
                    .with_removed_env(KIRO_REMOVED_ENV_KEYS)
            }
            SuperCliAgent::Codex => bail!("Codex is not a native external CLI launch target"),
        };
        if matches!(self.agent, SuperCliAgent::Gemini | SuperCliAgent::Copilot) {
            crate::remove_provider_secret_env(&mut child);
        }
        prepend_child_path(&mut child, overlay_home.join("bin"));
        clear_rtk_auto_wrap_control_env(&mut child);
        if presidio_enabled {
            child.extra_env.push((
                OsString::from("PRODEX_PRESIDIO_ENABLED"),
                OsString::from("1"),
            ));
        }
        Ok(RuntimeLaunchPlan::new(child).with_cleanup_path(overlay_home))
    }
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

fn runtime_super_gemini_cli_oauth_env(codex_home: &Path) -> Result<Vec<(OsString, OsString)>> {
    let secret = refresh_gemini_oauth_secret_if_needed(codex_home)?;
    let mut env = vec![(
        OsString::from("GOOGLE_CLOUD_ACCESS_TOKEN"),
        OsString::from(secret.access_token),
    )];
    if let Some(project_id) = secret.project_id.filter(|value| !value.trim().is_empty()) {
        env.push((
            OsString::from("GOOGLE_CLOUD_PROJECT"),
            OsString::from(project_id),
        ));
    }
    Ok(env)
}

fn runtime_super_gemini_cli_system_settings(overlay_home: &Path) -> Result<(PathBuf, PathBuf)> {
    let sources = prodex_runtime_gemini_cli_compat::gemini_settings_source_paths(None);
    let source = sources
        .iter()
        .find_map(|(name, path)| (name == "system").then(|| path.clone()));
    let defaults = sources
        .into_iter()
        .find_map(|(name, path)| (name == "system-defaults").then_some(path))
        .context("Gemini CLI system defaults path is unavailable")?;
    Ok((
        runtime_super_gemini_cli_system_settings_from(overlay_home, source.as_deref())?,
        defaults,
    ))
}

fn runtime_super_gemini_cli_system_settings_from(
    overlay_home: &Path,
    source: Option<&Path>,
) -> Result<PathBuf> {
    let mut settings = match source {
        Some(path)
            if path.try_exists().with_context(|| {
                format!(
                    "failed to inspect Gemini CLI system settings {}",
                    path.display()
                )
            })? =>
        {
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .with_context(|| {
                    format!(
                        "failed to open Gemini CLI system settings {}",
                        path.display()
                    )
                })?
                .take(GEMINI_SETTINGS_FILE_LIMIT + 1)
                .read_to_end(&mut bytes)
                .with_context(|| {
                    format!(
                        "failed to read Gemini CLI system settings {}",
                        path.display()
                    )
                })?;
            if bytes.len() as u64 > GEMINI_SETTINGS_FILE_LIMIT {
                bail!(
                    "Gemini CLI system settings exceed {} bytes: {}",
                    GEMINI_SETTINGS_FILE_LIMIT,
                    path.display()
                );
            }
            let text = std::str::from_utf8(&bytes).with_context(|| {
                format!(
                    "Gemini CLI system settings are not UTF-8: {}",
                    path.display()
                )
            })?;
            prodex_runtime_gemini_cli_compat::parse_gemini_settings_json(text).with_context(
                || format!("invalid Gemini CLI system settings: {}", path.display()),
            )?
        }
        _ => serde_json::json!({}),
    };
    let root = settings
        .as_object_mut()
        .context("Gemini CLI system settings must be a JSON object")?;
    let security = root
        .entry("security")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("Gemini CLI system setting `security` must be an object")?;
    let auth = security
        .entry("auth")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("Gemini CLI system setting `security.auth` must be an object")?;
    if let Some(enforced_type) = auth.get("enforcedType") {
        let enforced_type = enforced_type
            .as_str()
            .context("Gemini CLI `security.auth.enforcedType` must be a string")?;
        if enforced_type != "oauth-personal" {
            bail!(
                "Gemini CLI system policy enforces auth type `{enforced_type}`; imported Prodex profiles require `oauth-personal`"
            );
        }
    }
    auth.insert(
        "selectedType".to_string(),
        serde_json::Value::String("oauth-personal".to_string()),
    );

    let path = overlay_home.join("gemini-system-settings.json");
    let bytes = serde_json::to_vec_pretty(&settings)
        .context("failed to serialize Gemini CLI system settings")?;
    secret_store::write_private_file_atomic(&path, &bytes).with_context(|| {
        format!(
            "failed to write Gemini CLI system settings {}",
            path.display()
        )
    })?;
    Ok(path)
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

pub(super) fn handle_super_native_cli(args: SuperArgs, presidio_enabled: bool) -> Result<()> {
    let agent = args.cli.context("native external agent CLI is missing")?;
    match agent {
        SuperCliAgent::Gemini | SuperCliAgent::Agy
            if args.provider != Some(SuperExternalProvider::Gemini) =>
        {
            bail!("native Google agent CLIs require `gemini` or `--provider gemini`")
        }
        SuperCliAgent::Copilot if args.provider != Some(SuperExternalProvider::Copilot) => {
            bail!("native Copilot CLI requires `--provider copilot`")
        }
        SuperCliAgent::Kiro if args.provider.is_some() => {
            bail!("native Kiro CLI launch uses imported Kiro profiles directly; omit --provider")
        }
        _ => {}
    }
    if args.api_key.is_some() && agent != SuperCliAgent::Copilot {
        bail!("only native Copilot CLI supports Prodex --api-key routing")
    }
    execute_runtime_launch(SuperNativeCliLaunchStrategy {
        args,
        presidio_enabled,
        agent,
    })
}

#[cfg(test)]
#[path = "runtime_gemini_cli/tests/cases.rs"]
mod tests;
