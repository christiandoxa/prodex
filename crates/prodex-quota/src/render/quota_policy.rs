use super::{RuntimeQuotaPressureBand, RuntimeQuotaWindowStatus, RuntimeQuotaWindowSummary};

pub(super) fn quota_window_status(
    remaining_percent: i64,
    has_window: bool,
) -> RuntimeQuotaWindowStatus {
    #[cfg(feature = "mojo")]
    {
        runtime_quota_window_status_from_code(crate::mojo::window_status(
            remaining_percent,
            has_window,
        ))
    }

    #[cfg(not(feature = "mojo"))]
    {
        if !has_window {
            RuntimeQuotaWindowStatus::Unknown
        } else if remaining_percent == 0 {
            RuntimeQuotaWindowStatus::Exhausted
        } else if remaining_percent <= 5 {
            RuntimeQuotaWindowStatus::Critical
        } else if remaining_percent <= 15 {
            RuntimeQuotaWindowStatus::Thin
        } else {
            RuntimeQuotaWindowStatus::Ready
        }
    }
}

pub fn quota_pressure_band_from_windows(
    five_hour: RuntimeQuotaWindowSummary,
    weekly: RuntimeQuotaWindowSummary,
) -> RuntimeQuotaPressureBand {
    #[cfg(feature = "mojo")]
    {
        runtime_quota_pressure_band_from_code(crate::mojo::pressure_band(
            runtime_quota_window_status_code(five_hour.status),
            runtime_quota_window_status_code(weekly.status),
        ))
    }

    #[cfg(not(feature = "mojo"))]
    {
        [five_hour.status, weekly.status]
            .into_iter()
            .map(quota_pressure_band_from_window_status)
            .max()
            .unwrap_or(RuntimeQuotaPressureBand::Unknown)
    }
}

pub fn quota_pressure_band_from_window_status(
    status: RuntimeQuotaWindowStatus,
) -> RuntimeQuotaPressureBand {
    #[cfg(feature = "mojo")]
    {
        runtime_quota_pressure_band_from_code(crate::mojo::pressure_band(
            runtime_quota_window_status_code(status),
            runtime_quota_window_status_code(status),
        ))
    }

    #[cfg(not(feature = "mojo"))]
    {
        match status {
            RuntimeQuotaWindowStatus::Ready => RuntimeQuotaPressureBand::Healthy,
            RuntimeQuotaWindowStatus::Thin => RuntimeQuotaPressureBand::Thin,
            RuntimeQuotaWindowStatus::Critical => RuntimeQuotaPressureBand::Critical,
            RuntimeQuotaWindowStatus::Exhausted => RuntimeQuotaPressureBand::Exhausted,
            RuntimeQuotaWindowStatus::Unknown => RuntimeQuotaPressureBand::Unknown,
        }
    }
}

#[cfg(feature = "mojo")]
fn runtime_quota_window_status_code(status: RuntimeQuotaWindowStatus) -> i64 {
    match status {
        RuntimeQuotaWindowStatus::Ready => 0,
        RuntimeQuotaWindowStatus::Thin => 1,
        RuntimeQuotaWindowStatus::Critical => 2,
        RuntimeQuotaWindowStatus::Exhausted => 3,
        RuntimeQuotaWindowStatus::Unknown => 4,
    }
}

#[cfg(feature = "mojo")]
fn runtime_quota_window_status_from_code(code: i64) -> RuntimeQuotaWindowStatus {
    match code {
        0 => RuntimeQuotaWindowStatus::Ready,
        1 => RuntimeQuotaWindowStatus::Thin,
        2 => RuntimeQuotaWindowStatus::Critical,
        3 => RuntimeQuotaWindowStatus::Exhausted,
        _ => RuntimeQuotaWindowStatus::Unknown,
    }
}

#[cfg(feature = "mojo")]
fn runtime_quota_pressure_band_from_code(code: i64) -> RuntimeQuotaPressureBand {
    match code {
        0 => RuntimeQuotaPressureBand::Healthy,
        1 => RuntimeQuotaPressureBand::Thin,
        2 => RuntimeQuotaPressureBand::Critical,
        3 => RuntimeQuotaPressureBand::Exhausted,
        _ => RuntimeQuotaPressureBand::Unknown,
    }
}
