use anyhow::Result;

use crate::{
    AppPaths, AppState, AppStateIoExt, CleanupArgs, CleanupOlderThan,
    ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS, ProdexCleanupOptions,
    perform_prodex_cleanup_with_options, print_panel, runtime_proxy_log_dir,
};

pub(crate) fn handle_cleanup(args: CleanupArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let runtime_log_dir = runtime_proxy_log_dir();
    let cleanup_options = cleanup_options_from_args(&args);
    let summary = perform_prodex_cleanup_with_options(&paths, &mut state, cleanup_options)?;

    let fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "Orphan threshold".to_string(),
            cleanup_orphan_threshold_label(
                &args,
                cleanup_options.orphan_managed_profile_retention_seconds,
            ),
        ),
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
            "Chat history".to_string(),
            "left untouched (Codex-managed)".to_string(),
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

fn cleanup_options_from_args(args: &CleanupArgs) -> ProdexCleanupOptions {
    ProdexCleanupOptions {
        orphan_managed_profile_retention_seconds: if args.aggressive {
            0
        } else {
            args.older_than
                .map(CleanupOlderThan::seconds)
                .unwrap_or(ORPHAN_MANAGED_PROFILE_AUDIT_RETENTION_SECONDS)
        },
    }
}

fn cleanup_orphan_threshold_label(args: &CleanupArgs, seconds: i64) -> String {
    let mode = if args.aggressive {
        "aggressive"
    } else if args.older_than.is_some() {
        "explicit"
    } else {
        "default"
    };
    format!("{} ({mode})", cleanup_duration_label(seconds))
}

fn cleanup_duration_label(seconds: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;

    if seconds % DAY == 0 {
        return format!("{}d", seconds / DAY);
    }
    if seconds % HOUR == 0 {
        return format!("{}h", seconds / HOUR);
    }
    if seconds % MINUTE == 0 {
        return format!("{}m", seconds / MINUTE);
    }
    format!("{seconds}s")
}
