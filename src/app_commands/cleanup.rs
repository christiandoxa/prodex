use super::*;

pub(crate) fn handle_cleanup() -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let runtime_log_dir = runtime_proxy_log_dir();
    let summary = perform_prodex_cleanup(&paths, &mut state)?;

    let fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "Duplicate profiles".to_string(),
            summary.duplicate_profiles_removed.to_string(),
        ),
        (
            "Duplicate managed homes".to_string(),
            summary.duplicate_managed_profile_homes_removed.to_string(),
        ),
        (
            "Runtime logs".to_string(),
            format!(
                "{} removed from {}",
                summary.runtime_logs_removed,
                runtime_log_dir.display()
            ),
        ),
        (
            "Runtime pointer".to_string(),
            if summary.stale_runtime_log_pointer_removed > 0 {
                "removed stale latest-pointer file".to_string()
            } else {
                "clean".to_string()
            },
        ),
        (
            "Temp login homes".to_string(),
            summary.stale_login_dirs_removed.to_string(),
        ),
        (
            "Orphan managed homes".to_string(),
            summary.orphan_managed_profile_dirs_removed.to_string(),
        ),
        (
            "Transient root files".to_string(),
            summary.transient_root_files_removed.to_string(),
        ),
        (
            "Stale root temp files".to_string(),
            summary.stale_root_temp_files_removed.to_string(),
        ),
        (
            "Dead broker leases".to_string(),
            summary.dead_runtime_broker_leases_removed.to_string(),
        ),
        (
            "Dead broker registries".to_string(),
            summary.dead_runtime_broker_registries_removed.to_string(),
        ),
        (
            "Total removed".to_string(),
            summary.total_removed().to_string(),
        ),
    ];
    print_panel("Cleanup", &fields);
    Ok(())
}
