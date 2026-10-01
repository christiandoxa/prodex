use super::*;

pub fn window_label(seconds: Option<i64>) -> String {
    prodex_mojo_core::quota::quota_window_label(seconds)
        .expect("Mojo quota window label planner returned invalid output")
}

pub fn format_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M")
}

pub fn format_precise_reset_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S")
}

pub fn format_quota_snapshot_time(epoch: Option<i64>) -> String {
    format_local_epoch(epoch, "%Y-%m-%d %H:%M:%S")
}

fn format_local_epoch(epoch: Option<i64>, pattern: &str) -> String {
    let Some(epoch) = epoch else {
        return "-".to_string();
    };

    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|dt| dt.format(pattern).to_string())
        .unwrap_or_else(|| epoch.to_string())
}
