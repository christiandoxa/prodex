use super::*;
use anyhow::{Context, Result, bail};

pub(crate) fn handle_codex_logout(args: LogoutArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load_and_repair(&paths)?;
    repair_missing_active_profile_and_save(&paths, &mut state)?;
    let profile_name = resolve_profile_name(&state, args.selected_profile())?;
    let codex_home = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if !codex_home.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. `prodex logout` currently supports OpenAI/Codex profiles only.",
            profile_name,
            codex_home.provider.display_name()
        );
    }
    let codex_home = codex_home.codex_home.clone();

    let status = run_child_plan(
        &codex_child_plan(codex_home.clone(), vec![OsString::from("logout")]),
        None,
    )?;
    exit_with_status(status)
}
