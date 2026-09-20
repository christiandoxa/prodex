use super::*;
use crate::command_dispatch::command_exit_error;

pub(crate) fn super_uses_native_agy(args: &SuperArgs) -> bool {
    args.cli == Some(prodex_cli::SuperCliAgent::Agy)
}

pub(crate) fn handle_super_native_agy(args: SuperArgs) -> Result<()> {
    validate_native_agy_args(&args)?;
    let paths = AppPaths::discover()?;
    fs::create_dir_all(&paths.shared_codex_root)
        .with_context(|| format!("failed to create {}", paths.shared_codex_root.display()))?;

    let mut launch_args = args.codex_args.clone();
    if !launch_args
        .iter()
        .any(|arg| arg == "--dangerously-skip-permissions")
    {
        launch_args.insert(0, OsString::from("--dangerously-skip-permissions"));
    }
    if let Some(model) = args.local_model.as_deref()
        && !launch_args.iter().any(|arg| {
            arg.to_str().is_some_and(|arg| {
                matches!(arg, "--model" | "-m")
                    || arg.starts_with("--model=")
                    || arg.starts_with("-m=")
            })
        })
    {
        launch_args.splice(0..0, [OsString::from("--model"), OsString::from(model)]);
    }

    if args.dry_run {
        println!(
            "Prodex dry run: launch diagnostics\nFlow: native-cli\nProvider: antigravity\nProfile: (native CLI owned)\nRuntime proxy: disabled\n"
        );
        return Ok(());
    }

    let mut child =
        ChildProcessPlan::new(agy_bin(), paths.shared_codex_root).with_args(launch_args);
    crate::runtime_tools::clear_rtk_auto_wrap_control_env(&mut child);
    let status = run_child_plan(&child, None)?;
    if status.success() {
        Ok(())
    } else {
        Err(command_exit_error(
            child_exit_code(&status),
            "Antigravity CLI exited unsuccessfully",
        ))
    }
}

fn validate_native_agy_args(args: &SuperArgs) -> Result<()> {
    if args.provider != Some(prodex_cli::SuperExternalProvider::Gemini) {
        bail!("--cli agy requires --provider gemini; use prodex s gemini --cli agy");
    }
    if args.presidio {
        bail!("--presidio is unsupported for native Antigravity");
    }
    if args.api_key.is_some()
        || args.sub_agent
        || !args.tools.is_empty()
        || !args.required_tools.is_empty()
        || args.auto_rotate
        || args.auto_redeem
        || args.skip_quota_check
        || args.base_url.is_some()
        || args.no_proxy
        || args.url.is_some()
        || args.local_context_window.is_some()
        || args.local_auto_compact_token_limit.is_some()
        || !args.codex_features.to_codex_config_args().is_empty()
        || prodex_runtime_launch::codex_resume_requested(&args.codex_args)
    {
        bail!("selected options are unsupported for native Antigravity");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antigravity_compatibility_requires_gemini_provider_and_rejects_presidio() {
        let crate::Commands::Super(args) = crate::parse_cli_command_from([
            "prodex",
            "s",
            "gemini",
            "--cli",
            "agy",
            "--no-presidio",
        ])
        .unwrap() else {
            panic!("expected Super command");
        };
        assert!(super_uses_native_agy(&args));
        validate_native_agy_args(&args).unwrap();

        let crate::Commands::Super(args) =
            crate::parse_cli_command_from(["prodex", "s", "gemini", "--cli", "agy", "--presidio"])
                .unwrap()
        else {
            panic!("expected Super command");
        };
        assert!(
            validate_native_agy_args(&args)
                .unwrap_err()
                .to_string()
                .contains("--presidio is unsupported")
        );
    }
}
