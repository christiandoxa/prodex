use anyhow::Result;
use std::path::Path;

pub(crate) use prodex_runtime_gemini_cli_compat::{
    GeminiSettingsSource, gemini_cli_config_home_for, gemini_settings_source_paths_for,
    gemini_settings_sources, parse_gemini_settings_json,
};

pub(crate) fn prepare_gemini_cli_compat(codex_home: &Path) -> Result<()> {
    prodex_runtime_gemini_cli_compat::prepare_gemini_cli_compat(codex_home)
}

pub(crate) fn handle_gemini_compat_refresh(args: crate::GeminiCompatRefreshArgs) -> Result<()> {
    prepare_gemini_cli_compat(&args.codex_home)?;
    println!(
        "Gemini CLI compatibility refreshed in {}",
        args.codex_home.display()
    );
    Ok(())
}
