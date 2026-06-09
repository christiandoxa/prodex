use super::*;

const PRODEX_GEMINI_PLACEHOLDER_TOKEN: &str = "prodex-runtime-provider";

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
            skip_quota_check: true,
            base_url: self
                .args
                .base_url
                .as_deref()
                .or(Some(SUPER_GEMINI_DEFAULT_BASE_URL)),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: false,
            smart_context_enabled: true,
            presidio_redaction_enabled: self.presidio_enabled,
            model_context_window_tokens: self.args.local_context_window.map(|value| value as u64),
            gemini_thinking_budget_tokens: None,
            force_runtime_proxy: false,
            model_provider_override: (self.agent == SuperCliAgent::Gemini)
                .then_some(SUPER_GEMINI_PROVIDER_ID),
            profile_v2_name: None,
            external_provider: (self.agent == SuperCliAgent::Gemini).then_some("gemini-oauth"),
            external_provider_api_key: None,
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        let caveman_home = prepare_caveman_launch_home(&prepared.paths, &prepared.codex_home)?;
        prodex_caveman_assets::configure_rtk_codex_home(&caveman_home)?;
        prodex_caveman_assets::configure_super_optimizer_codex_home(&caveman_home)?;

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
                ChildProcessPlan::new(gemini_bin(), prepared.codex_home.clone())
                    .with_args(launch_args)
                    .with_extra_env(vec![
                        (
                            OsString::from("GOOGLE_GENAI_USE_GCA"),
                            OsString::from("true"),
                        ),
                        (
                            OsString::from("GOOGLE_CLOUD_ACCESS_TOKEN"),
                            OsString::from(PRODEX_GEMINI_PLACEHOLDER_TOKEN),
                        ),
                        (
                            OsString::from("CODE_ASSIST_ENDPOINT"),
                            OsString::from(proxy_base_url),
                        ),
                        (
                            OsString::from("CODE_ASSIST_API_VERSION"),
                            OsString::from("v1internal"),
                        ),
                    ])
            }
            SuperCliAgent::Agy => {
                ChildProcessPlan::new(agy_bin(), prepared.codex_home.clone()).with_args(launch_args)
            }
            SuperCliAgent::Codex => bail!("Codex is not a native Google CLI launch target"),
        };
        prepend_child_path(&mut child, caveman_home.join("bin"));
        clear_rtk_auto_wrap_control_env(&mut child);
        if self.presidio_enabled {
            child.extra_env.push((
                OsString::from("PRODEX_PRESIDIO_ENABLED"),
                OsString::from("1"),
            ));
        }
        Ok(RuntimeLaunchPlan::new(child).with_cleanup_path(caveman_home))
    }
}

fn runtime_super_google_cli_launch_args(
    agent: SuperCliAgent,
    args: &[OsString],
    model: Option<&str>,
) -> Vec<OsString> {
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

pub(super) fn handle_super_google_cli(args: SuperArgs, presidio_enabled: bool) -> Result<()> {
    if args.provider != Some(SuperExternalProvider::Gemini) {
        bail!("native Google agent CLIs require `gemini` or `--provider gemini`")
    }
    let agent = args.cli.context("native Google agent CLI is missing")?;
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
}
