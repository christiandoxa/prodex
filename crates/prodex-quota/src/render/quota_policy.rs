use super::{RuntimeQuotaPressureBand, RuntimeQuotaWindowStatus, RuntimeQuotaWindowSummary};

pub(super) fn quota_window_status(
    remaining_percent: i64,
    has_window: bool,
) -> RuntimeQuotaWindowStatus {
    runtime_quota_window_status_from_code(crate::mojo::window_status(remaining_percent, has_window))
}

pub fn quota_pressure_band_from_windows(
    five_hour: RuntimeQuotaWindowSummary,
    weekly: RuntimeQuotaWindowSummary,
) -> RuntimeQuotaPressureBand {
    runtime_quota_pressure_band_from_code(crate::mojo::pressure_band(
        runtime_quota_window_status_code(five_hour.status),
        runtime_quota_window_status_code(weekly.status),
    ))
}

pub fn quota_pressure_band_from_window_status(
    status: RuntimeQuotaWindowStatus,
) -> RuntimeQuotaPressureBand {
    runtime_quota_pressure_band_from_code(crate::mojo::pressure_band(
        runtime_quota_window_status_code(status),
        runtime_quota_window_status_code(status),
    ))
}

fn runtime_quota_window_status_code(status: RuntimeQuotaWindowStatus) -> i64 {
    match status {
        RuntimeQuotaWindowStatus::Ready => 0,
        RuntimeQuotaWindowStatus::Thin => 1,
        RuntimeQuotaWindowStatus::Critical => 2,
        RuntimeQuotaWindowStatus::Exhausted => 3,
        RuntimeQuotaWindowStatus::Unknown => 4,
    }
}

fn runtime_quota_window_status_from_code(code: i64) -> RuntimeQuotaWindowStatus {
    match code {
        0 => RuntimeQuotaWindowStatus::Ready,
        1 => RuntimeQuotaWindowStatus::Thin,
        2 => RuntimeQuotaWindowStatus::Critical,
        3 => RuntimeQuotaWindowStatus::Exhausted,
        _ => RuntimeQuotaWindowStatus::Unknown,
    }
}

fn runtime_quota_pressure_band_from_code(code: i64) -> RuntimeQuotaPressureBand {
    match code {
        0 => RuntimeQuotaPressureBand::Healthy,
        1 => RuntimeQuotaPressureBand::Thin,
        2 => RuntimeQuotaPressureBand::Critical,
        3 => RuntimeQuotaPressureBand::Exhausted,
        _ => RuntimeQuotaPressureBand::Unknown,
    }
}
