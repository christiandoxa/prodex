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

pub fn format_reset_credits(credits: &crate::RateLimitResetCreditsSummary) -> String {
    // Local timezone conversion stays at the host/Chrono boundary.
    let timestamp = match credits.next_expiry {
        crate::RateLimitResetCreditExpiry::At(epoch) => Local
            .timestamp_opt(epoch, 0)
            .single()
            .map(|value| value.format("%Y-%m-%d %H:%M:%S %:z").to_string())
            .unwrap_or_default(),
        _ => String::new(),
    };
    prodex_mojo_core::quota::quota_reset_credit_text(
        credits.available_count,
        credits.next_expiry,
        &timestamp,
    )
    .expect("Mojo reset-credit formatter returned invalid output")
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
