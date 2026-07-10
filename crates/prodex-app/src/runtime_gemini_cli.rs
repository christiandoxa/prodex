use crate::{
    PreparedRuntimeLaunch, RuntimeLaunchRequest, RuntimeLaunchStrategy, RuntimeProxyEndpoint,
    agy_bin, clear_rtk_auto_wrap_control_env, execute_runtime_launch, gemini_bin, kiro_bin,
    prepare_prodex_overlay_home, prepend_child_path, read_kiro_auth_secret,
    refresh_gemini_oauth_secret_if_needed, write_kiro_cli_data_dir,
};
use anyhow::{Context, Result, bail};
use prodex_cli::{
    SUPER_GEMINI_DEFAULT_BASE_URL, SUPER_GEMINI_PROVIDER_ID, SuperArgs, SuperCliAgent,
    SuperExternalProvider,
};
use prodex_runtime_launch::{ChildProcessPlan, RuntimeLaunchPlan};
use std::ffi::OsString;
use std::path::Path;

struct SuperGeminiCliLaunchStrategy {
    args: SuperArgs,
    presidio_enabled: bool,
    agent: SuperCliAgent,
}

impl RuntimeLaunchStrategy for SuperGeminiCliLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
            auto_redeem: false,
            skip_quota_check: true,
            base_url: self
                .args
                .base_url
                .as_deref()
                .or((self.agent == SuperCliAgent::Gemini).then_some(SUPER_GEMINI_DEFAULT_BASE_URL)),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: false,
            smart_context_enabled: self.agent == SuperCliAgent::Gemini,
            presidio_redaction_enabled: self.presidio_enabled
                && self.agent == SuperCliAgent::Gemini,
            model_context_window_tokens: self.args.local_context_window.map(|value| value as u64),
            gemini_thinking_budget_tokens: None,
            force_runtime_proxy: false,
            model_provider_override: (self.agent == SuperCliAgent::Gemini)
                .then_some(SUPER_GEMINI_PROVIDER_ID),
            profile_v2_name: None,
            external_provider: match self.agent {
                SuperCliAgent::Gemini => Some("gemini-oauth"),
                SuperCliAgent::Kiro => Some("kiro"),
                SuperCliAgent::Agy | SuperCliAgent::Codex => None,
            },
            external_provider_api_key: None,
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        if self.presidio_enabled {
            crate::ensure_presidio_services_for_super_launch(&prepared.paths)?;
        }
        let overlay_home = prepare_prodex_overlay_home(&prepared.paths, &prepared.codex_home)?;
        prodex_caveman_assets::configure_rtk_codex_home(&overlay_home)?;
        prodex_caveman_assets::configure_super_optimizer_codex_home_with_presidio(
            &overlay_home,
            self.presidio_enabled,
        )?;

        let launch_args = runtime_super_google_cli_launch_args(
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
            SuperCliAgent::Agy => {
                ChildProcessPlan::new(agy_bin(), prepared.codex_home.clone()).with_args(launch_args)
            }
            SuperCliAgent::Kiro => ChildProcessPlan::new(kiro_bin(), prepared.codex_home.clone())
                .with_args(launch_args)
                .with_extra_env(runtime_super_kiro_cli_profile_env(
                    &prepared.codex_home,
                    &overlay_home,
                )?),
            SuperCliAgent::Codex => bail!("Codex is not a native Google CLI launch target"),
        };
        prepend_child_path(&mut child, overlay_home.join("bin"));
        clear_rtk_auto_wrap_control_env(&mut child);
        if self.presidio_enabled {
            child.extra_env.push((
                OsString::from("PRODEX_PRESIDIO_ENABLED"),
                OsString::from("1"),
            ));
        }
        Ok(RuntimeLaunchPlan::new(child).with_cleanup_path(overlay_home))
    }
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

fn runtime_super_google_cli_launch_args(
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

pub(super) fn handle_super_google_cli(args: SuperArgs, presidio_enabled: bool) -> Result<()> {
    let agent = args.cli.context("native Google agent CLI is missing")?;
    match agent {
        SuperCliAgent::Gemini | SuperCliAgent::Agy
            if args.provider != Some(SuperExternalProvider::Gemini) =>
        {
            bail!("native Google agent CLIs require `gemini` or `--provider gemini`")
        }
        SuperCliAgent::Kiro if args.provider.is_some() => {
            bail!("native Kiro CLI launch uses imported Kiro profiles directly; omit --provider")
        }
        _ => {}
    }
    if args.api_key.is_some() {
        bail!("native Google agent CLIs do not support Prodex --api-key routing")
    }
    execute_runtime_launch(SuperGeminiCliLaunchStrategy {
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
            runtime_super_google_cli_launch_args(
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
            runtime_super_google_cli_launch_args(SuperCliAgent::Gemini, &args, None),
            args
        );
    }

    #[test]
    fn native_agy_defaults_to_dangerously_skip_permissions() {
        assert_eq!(
            runtime_super_google_cli_launch_args(
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
            runtime_super_google_cli_launch_args(
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
            runtime_super_google_cli_launch_args(SuperCliAgent::Kiro, &args, Some("ignored")),
            args
        );
    }

    #[test]
    fn native_kiro_cli_runtime_request_skips_proxy_features() {
        let strategy = SuperGeminiCliLaunchStrategy {
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
}
