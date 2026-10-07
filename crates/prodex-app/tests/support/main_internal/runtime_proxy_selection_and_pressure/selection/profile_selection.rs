use super::*;

#[path = "profile_selection/commands.rs"]
mod commands;

#[test]
fn runtime_probe_refresh_wait_spans_multiple_short_slices_until_progress() {
    let probe_refresh = RuntimeProbeRefreshTestGuard::new();
    let observed_revision = probe_refresh.observed_revision();
    let notify = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(30));
        note_runtime_probe_refresh_progress();
    });

    assert!(
        wait_for_runtime_probe_refresh_progress(
            std::time::Duration::from_millis(200),
            std::time::Duration::from_millis(5),
            observed_revision,
        ),
        "probe refresh wait should survive several short timeout slices before progress"
    );
    notify.join().expect("probe refresh notifier should join");
}
