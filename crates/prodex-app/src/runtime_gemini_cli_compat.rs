use anyhow::Result;
use std::path::Path;
use terminal_ui::print_panel;

pub(crate) use prodex_runtime_gemini_cli_compat::{
    GeminiSettingsSource, gemini_settings_source_paths_for_config_home, gemini_settings_sources,
    gemini_settings_sources_for_config_home, parse_gemini_settings_json,
};

pub(crate) fn prepare_gemini_cli_compat(codex_home: &Path) -> Result<()> {
    prodex_runtime_gemini_cli_compat::prepare_gemini_cli_compat(codex_home)
}

pub(crate) fn handle_gemini_compat_refresh(args: crate::GeminiCompatRefreshArgs) -> Result<()> {
    prepare_gemini_cli_compat(&args.codex_home)?;
    print_gemini_compat_refresh_status(&args.codex_home)?;
    Ok(())
}

fn print_gemini_compat_refresh_status(codex_home: &Path) -> Result<()> {
    print_panel(
        "Gemini CLI Compatibility",
        &gemini_compat_status_fields(codex_home),
    )?;
    Ok(())
}

fn gemini_compat_status_fields(codex_home: &Path) -> Vec<(String, String)> {
    vec![("CODEX_HOME".to_string(), codex_home.display().to_string())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn gemini_compat_status_fields_contain_codex_home() {
        let codex_home = PathBuf::from("/tmp/prodex-gemini");
        let fields = gemini_compat_status_fields(&codex_home);

        assert!(fields.contains(&("CODEX_HOME".to_string(), "/tmp/prodex-gemini".to_string())));
    }
}
