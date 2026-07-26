use anyhow::{Result, bail};

use crate::{
    CodexRuntimeFeatureArgs, GuiArgs, RuntimeToolArgs, SuperArgs, SuperCliAgent, handle_desktop_gui,
};

pub(crate) fn handle_gui(args: GuiArgs) -> Result<()> {
    handle_desktop_gui(
        RuntimeToolArgs {
            profile: args.profile,
            auto_rotate: args.auto_rotate,
            no_auto_rotate: args.no_auto_rotate,
            auto_redeem: args.auto_redeem,
            skip_quota_check: args.skip_quota_check,
            full_access: false,
            dry_run: false,
            base_url: args.base_url,
            no_proxy: args.no_proxy,
            smart_context: false,
            super_mode: false,
            tools: Vec::new(),
            required_tools: Vec::new(),
            presidio: false,
            external_provider: None,
            external_provider_api_key: None,
            harness: None,
            codex_features: CodexRuntimeFeatureArgs::default(),
            codex_args: Vec::new(),
        },
        false,
    )
}

pub(crate) fn handle_super_gui(args: SuperArgs) -> Result<()> {
    if args.dry_run {
        bail!("--dry-run is not supported with Codex Desktop")
    }
    if !matches!(args.cli, None | Some(SuperCliAgent::Codex)) {
        bail!("`prodex s gui` supports only the Codex desktop app")
    }
    let use_presidio = match args.presidio_preference() {
        Some(use_presidio) => use_presidio,
        None => super::prompt_super_presidio_opt_in()?,
    };
    handle_desktop_gui(
        args.into_runtime_tool_args_with_presidio(use_presidio),
        true,
    )
}
