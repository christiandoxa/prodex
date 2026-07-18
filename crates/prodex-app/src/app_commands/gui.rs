use anyhow::{Result, bail};

use crate::{
    CavemanArgs, CodexRuntimeFeatureArgs, GuiArgs, SuperArgs, SuperCliAgent, handle_desktop_gui,
};

pub(crate) fn handle_gui(args: GuiArgs) -> Result<()> {
    handle_desktop_gui(
        CavemanArgs {
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
            super_optimizer_overlay: false,
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
    handle_desktop_gui(args.into_caveman_args_with_presidio(use_presidio), true)
}
