use anyhow::Result;
use prodex_cli::Commands;
use terminal_ui::print_stderr_panel;

pub(crate) fn show_update_notice_if_available(command: &Commands) -> Result<()> {
    if !prodex_update_notice::should_emit_update_notice(command) {
        return Ok(());
    }

    let paths = prodex_core::AppPaths::discover()?;
    let prodex_update_notice::ProdexVersionStatus::UpdateAvailable(latest_version) =
        prodex_update_notice::prodex_version_status(&paths)?
    else {
        return Ok(());
    };

    let current_version = prodex_update_notice::current_prodex_version();
    let update_command = prodex_update_notice::prodex_update_command_for_version(&latest_version);
    let install_warning = prodex_update_notice::current_prodex_install_warning();
    let mut lines = vec![
        format!(
            "A newer prodex release is available: {} -> {}",
            current_version, latest_version
        ),
        format!("Update with: {update_command}"),
    ];
    if let Some(warning) = install_warning {
        lines.push(format!("WARNING: {warning}"));
    }
    print_stderr_panel("Update Available", &lines)?;
    Ok(())
}
