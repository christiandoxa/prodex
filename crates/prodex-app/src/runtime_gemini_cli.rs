use crate::{
    PreparedRuntimeLaunch, RuntimeLaunchRequest, RuntimeLaunchStrategy, RuntimeProxyEndpoint,
    agy_bin, clear_rtk_auto_wrap_control_env, copilot_bin, execute_runtime_launch, gemini_bin,
    kiro_bin, prepare_prodex_overlay_home, prepend_child_path, read_kiro_auth_secret,
    refresh_gemini_oauth_secret_if_needed, write_kiro_cli_data_dir,
};
use anyhow::{Context, Result, bail};
use prodex_cli::{
    SUPER_COPILOT_DEFAULT_MODEL, SUPER_COPILOT_PROVIDER_ID, SUPER_GEMINI_DEFAULT_BASE_URL,
    SUPER_GEMINI_PROVIDER_ID, SuperArgs, SuperCliAgent, SuperExternalProvider,
};
use prodex_runtime_launch::{ChildProcessPlan, RuntimeLaunchPlan, local_proxy_bypass_env};
use std::ffi::OsString;
use std::path::Path;

const PRODEX_COPILOT_PROXY_API_KEY: &str = "prodex-runtime-provider";

struct SuperNativeCliLaunchStrategy {
    args: SuperArgs,
    presidio_enabled: bool,
    agent: SuperCliAgent,
}

impl RuntimeLaunchStrategy for SuperNativeCliLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
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
                SuperCliAgent::Agy | SuperCliAgent::Codex => None,
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
        let overlay_home = prepare_prodex_overlay_home(&prepared.paths, &prepared.codex_home)?;
        prodex_caveman_assets::configure_rtk_codex_home(&overlay_home)?;
        prodex_caveman_assets::configure_super_optimizer_codex_home_with_presidio(
            &overlay_home,
            presidio_enabled,
        )?;

        let launch_args = runtime_super_native_cli_launch_args(
            self.agent,
            &self.args.codex_args,
            self.args.local_model.as_deref(),
        );
        let mut child = match self.agent {
            SuperCliAgent::Gemini => {
                let runtime_proxy =
                    runtime_proxy.context("Gemini CLI launch requires a local runtime proxy")?;
                let proxy_base_url = format!("http://{}", runtime_proxy.listen_addr);
                let gemini_auth_env = runtime_super_gemini_cli_oauth_env(&prepared.codex_home)?;
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
                    ]))
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
            SuperCliAgent::Agy => {
                ChildProcessPlan::new(agy_bin(), prepared.codex_home.clone()).with_args(launch_args)
            }
            SuperCliAgent::Kiro => ChildProcessPlan::new(kiro_bin(), prepared.codex_home.clone())
                .with_args(launch_args)
                .with_extra_env(runtime_super_kiro_cli_profile_env(
                    &prepared.codex_home,
                    &overlay_home,
                )?),
            SuperCliAgent::Codex => bail!("Codex is not a native external CLI launch target"),
        };
        if self.agent == SuperCliAgent::Copilot {
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
                matches!(
                    arg.to_str(),
                    Some("--yolo" | "-y" | "--approval-mode" | "--approval-mode=yolo")
                )
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
        && !launch_args
            .iter()
            .any(|arg| matches!(arg.to_str(), Some("--model" | "-m")))
    {
        launch_args.splice(0..0, [OsString::from("--model"), OsString::from(model)]);
    }
    launch_args
}

fn runtime_super_kiro_cli_launch_args(args: &[OsString], model: Option<&str>) -> Vec<OsString> {
    if model.is_none()
        || args
            .iter()
            .any(|arg| matches!(arg.to_str(), Some("--model" | "-m")))
    {
        return args.to_vec();
    }
    let mut launch_args = Vec::with_capacity(args.len() + 3);
    launch_args.push(OsString::from("chat"));
    launch_args.push(OsString::from("--model"));
    launch_args.push(OsString::from(model.unwrap_or_default()));
    launch_args.extend(args.iter().cloned());
    launch_args
}

fn runtime_super_kiro_cli_profile_env(
    codex_home: &Path,
    overlay_home: &Path,
) -> Result<Vec<(OsString, OsString)>> {
    let secret = read_kiro_auth_secret(codex_home)?;
    let data_dir = overlay_home.join("kiro-data");
    write_kiro_cli_data_dir(&data_dir, &secret)?;
    let mut env = vec![(OsString::from("Q_CLI_DATA_DIR"), data_dir.into_os_string())];
    if let Some(region) = secret.region.filter(|value| !value.trim().is_empty()) {
        env.push((OsString::from("AWS_REGION"), OsString::from(region)));
    }
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
mod tests {
    use super::*;
    use crate::{GeminiOAuthSecret, write_gemini_oauth_secret};
    use prodex_cli::CodexRuntimeFeatureArgs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn native_cli_super_args() -> SuperArgs {
        SuperArgs {
            profile: Some("kiro-main".to_string()),
            auto_rotate: false,
            no_auto_rotate: false,
            auto_redeem: false,
            skip_quota_check: false,
            dry_run: false,
            base_url: None,
            no_proxy: false,
            presidio: false,
            no_presidio: false,
            url: None,
            provider: None,
            harness: None,
            cli: None,
            api_key: None,
            local_model: None,
            local_context_window: None,
            local_auto_compact_token_limit: None,
            codex_features: CodexRuntimeFeatureArgs::default(),
            codex_args: Vec::new(),
        }
    }

    #[test]
    fn native_gemini_cli_defaults_to_yolo_and_forwards_model() {
        assert_eq!(
            runtime_super_native_cli_launch_args(
                SuperCliAgent::Gemini,
                &[OsString::from("review")],
                Some("gemini-test"),
            ),
            vec![
                OsString::from("--model"),
                OsString::from("gemini-test"),
                OsString::from("--yolo"),
                OsString::from("review"),
            ]
        );
    }

    #[test]
    fn native_gemini_cli_keeps_explicit_approval_mode() {
        let args = [OsString::from("--approval-mode"), OsString::from("plan")];
        assert_eq!(
            runtime_super_native_cli_launch_args(SuperCliAgent::Gemini, &args, None),
            args
        );
    }

    #[test]
    fn native_agy_defaults_to_dangerously_skip_permissions() {
        assert_eq!(
            runtime_super_native_cli_launch_args(
                SuperCliAgent::Agy,
                &[OsString::from("--continue")],
                None,
            ),
            vec![
                OsString::from("--dangerously-skip-permissions"),
                OsString::from("--continue"),
            ]
        );
    }

    #[test]
    fn native_copilot_cli_forwards_model_without_google_flags() {
        assert_eq!(
            runtime_super_native_cli_launch_args(
                SuperCliAgent::Copilot,
                &[OsString::from("--prompt"), OsString::from("review")],
                Some("gpt-test"),
            ),
            vec![
                OsString::from("--model"),
                OsString::from("gpt-test"),
                OsString::from("--prompt"),
                OsString::from("review"),
            ]
        );
    }

    #[test]
    fn native_copilot_cli_uses_local_responses_provider_contract() {
        let endpoint = RuntimeProxyEndpoint {
            listen_addr: "127.0.0.1:48123".parse().unwrap(),
            openai_mount_path: "/v1".to_string(),
            local_model_provider_id: Some(SUPER_COPILOT_PROVIDER_ID.to_string()),
            realtime_ws_base_url: None,
            realtime_ws_model: None,
            lease_dir: std::env::temp_dir(),
            broker_session_affinity_control: None,
            _lease: None,
            _direct_proxy: None,
        };
        let env = runtime_super_copilot_cli_env(&endpoint, "gpt-test");
        let value = |key: &str| {
            env.iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.to_string_lossy().into_owned())
        };

        assert_eq!(
            value("COPILOT_PROVIDER_BASE_URL").as_deref(),
            Some("http://127.0.0.1:48123/v1")
        );
        assert_eq!(value("COPILOT_PROVIDER_TYPE").as_deref(), Some("openai"));
        assert_eq!(
            value("COPILOT_PROVIDER_WIRE_API").as_deref(),
            Some("responses")
        );
        assert_eq!(value("COPILOT_PROVIDER_TRANSPORT").as_deref(), Some("http"));
        assert_eq!(value("COPILOT_MODEL").as_deref(), Some("gpt-test"));
        assert_eq!(
            value("COPILOT_PROVIDER_API_KEY").as_deref(),
            Some(PRODEX_COPILOT_PROXY_API_KEY)
        );
    }

    #[test]
    fn native_gemini_cli_uses_profile_oauth_token_env() {
        let home = std::env::temp_dir().join(format!(
            "prodex-native-gemini-cli-oauth-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let secret = GeminiOAuthSecret {
            auth_mode: "gemini_oauth".to_string(),
            access_token: "profile-access-token".to_string(),
            refresh_token: Some("profile-refresh-token".to_string()),
            token_type: Some("Bearer".to_string()),
            scope: None,
            expiry_date: None,
            email: "gemini-user@example.com".to_string(),
            project_id: Some("profile-project".to_string()),
        };
        write_gemini_oauth_secret(&home, &secret).expect("secret should write");

        let env = runtime_super_gemini_cli_oauth_env(&home).expect("env should build");
        assert_eq!(
            env.iter()
                .find(|(key, _)| key == "GOOGLE_CLOUD_ACCESS_TOKEN")
                .map(|(_, value)| value.as_os_str()),
            Some(std::ffi::OsStr::new("profile-access-token"))
        );
        assert_eq!(
            env.iter()
                .find(|(key, _)| key == "GOOGLE_CLOUD_PROJECT")
                .map(|(_, value)| value.as_os_str()),
            Some(std::ffi::OsStr::new("profile-project"))
        );

        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn native_kiro_cli_injects_chat_model_when_needed() {
        assert_eq!(
            runtime_super_native_cli_launch_args(
                SuperCliAgent::Kiro,
                &[OsString::from("review this repo")],
                Some("claude-4-sonnet"),
            ),
            vec![
                OsString::from("chat"),
                OsString::from("--model"),
                OsString::from("claude-4-sonnet"),
                OsString::from("review this repo"),
            ]
        );
    }

    #[test]
    fn native_kiro_cli_keeps_explicit_model_flag() {
        let args = [OsString::from("--model"), OsString::from("existing-model")];
        assert_eq!(
            runtime_super_native_cli_launch_args(SuperCliAgent::Kiro, &args, Some("ignored")),
            args
        );
    }

    #[test]
    fn native_kiro_cli_runtime_request_skips_proxy_features() {
        let strategy = SuperNativeCliLaunchStrategy {
            args: native_cli_super_args(),
            presidio_enabled: true,
            agent: SuperCliAgent::Kiro,
        };
        let request = strategy.runtime_request();
        assert_eq!(request.external_provider, Some("kiro"));
        assert!(!request.smart_context_enabled);
        assert!(!request.presidio_redaction_enabled);
        assert_eq!(request.base_url, None);
    }

    #[test]
    fn native_copilot_cli_runtime_request_enables_provider_proxy() {
        let mut args = native_cli_super_args();
        args.profile = Some("copilot-main".to_string());
        args.provider = Some(SuperExternalProvider::Copilot);
        args.api_key = Some("provider-test-key".to_string());
        let strategy = SuperNativeCliLaunchStrategy {
            args,
            presidio_enabled: true,
            agent: SuperCliAgent::Copilot,
        };

        let request = strategy.runtime_request();
        assert_eq!(request.external_provider, Some("copilot"));
        assert_eq!(request.external_provider_api_key, Some("provider-test-key"));
        assert_eq!(
            request.model_provider_override,
            Some(SUPER_COPILOT_PROVIDER_ID)
        );
        assert_eq!(
            request.base_url,
            Some(SuperExternalProvider::Copilot.default_base_url())
        );
        assert!(request.smart_context_enabled);
        assert!(request.presidio_redaction_enabled);
    }
}
