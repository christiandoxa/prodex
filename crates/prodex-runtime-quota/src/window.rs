use chrono::Local;
use prodex_quota::{
    RuntimeQuotaWindowStatus, RuntimeQuotaWindowSummary, UsageResponse, find_main_window,
    openai_quota_runtime_window_pair, remaining_percent,
};
use runtime_proxy_crate as runtime_proxy;

pub fn runtime_quota_window_status_to_proxy(
    status: RuntimeQuotaWindowStatus,
) -> runtime_proxy::RuntimeSelectionQuotaWindowStatus {
    match status {
        RuntimeQuotaWindowStatus::Ready => runtime_proxy::RuntimeSelectionQuotaWindowStatus::Ready,
        RuntimeQuotaWindowStatus::Thin => runtime_proxy::RuntimeSelectionQuotaWindowStatus::Thin,
        RuntimeQuotaWindowStatus::Critical => {
            runtime_proxy::RuntimeSelectionQuotaWindowStatus::Critical
        }
        RuntimeQuotaWindowStatus::Exhausted => {
            runtime_proxy::RuntimeSelectionQuotaWindowStatus::Exhausted
        }
        RuntimeQuotaWindowStatus::Unknown => {
            runtime_proxy::RuntimeSelectionQuotaWindowStatus::Unknown
        }
    }
}

pub fn runtime_quota_window_status_from_proxy(
    status: runtime_proxy::RuntimeSelectionQuotaWindowStatus,
) -> RuntimeQuotaWindowStatus {
    match status {
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Ready => RuntimeQuotaWindowStatus::Ready,
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Thin => RuntimeQuotaWindowStatus::Thin,
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Critical => {
            RuntimeQuotaWindowStatus::Critical
        }
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Exhausted => {
            RuntimeQuotaWindowStatus::Exhausted
        }
        runtime_proxy::RuntimeSelectionQuotaWindowStatus::Unknown => {
            RuntimeQuotaWindowStatus::Unknown
        }
    }
}

pub fn runtime_quota_window_summary_to_proxy(
    window: RuntimeQuotaWindowSummary,
) -> runtime_proxy::RuntimeProxyQuotaWindowSummary {
    runtime_proxy::RuntimeProxyQuotaWindowSummary {
        status: runtime_quota_window_status_to_proxy(window.status),
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub fn runtime_quota_window_summary_from_proxy(
    window: runtime_proxy::RuntimeProxyQuotaWindowSummary,
) -> RuntimeQuotaWindowSummary {
    RuntimeQuotaWindowSummary {
        status: runtime_quota_window_status_from_proxy(window.status),
        remaining_percent: window.remaining_percent,
        reset_at: window.reset_at,
    }
}

pub fn runtime_quota_window_observation(
    usage: &UsageResponse,
    label: &str,
) -> Option<runtime_proxy::RuntimeProxyQuotaWindowObservation> {
    runtime_quota_window_observation_at(usage, label, Local::now().timestamp())
}

pub fn runtime_quota_window_observation_at(
    usage: &UsageResponse,
    label: &str,
    now: i64,
) -> Option<runtime_proxy::RuntimeProxyQuotaWindowObservation> {
    let window = find_main_window(openai_quota_runtime_window_pair(usage)?, label)?;
    let remaining_percent = remaining_percent(window.used_percent);
    let reset_at = window.reset_at.unwrap_or(i64::MAX);
    let seconds_until_reset = if reset_at == i64::MAX {
        i64::MAX
    } else {
        (reset_at - now).max(0)
    };
    let pressure_score = seconds_until_reset
        .saturating_mul(1_000)
        .checked_div(remaining_percent.max(1))
        .unwrap_or(i64::MAX);

    Some(runtime_proxy::RuntimeProxyQuotaWindowObservation {
        remaining_percent,
        reset_at,
        pressure_score,
    })
}

pub fn runtime_quota_window_status_reason(status: RuntimeQuotaWindowStatus) -> &'static str {
    runtime_proxy::runtime_proxy_quota_window_status_reason(runtime_quota_window_status_to_proxy(
        status,
    ))
}

pub fn runtime_quota_window_summary(
    observation: Option<runtime_proxy::RuntimeProxyQuotaWindowObservation>,
) -> RuntimeQuotaWindowSummary {
    runtime_quota_window_summary_from_proxy(runtime_proxy::runtime_proxy_quota_window_summary(
        observation,
    ))
}

pub fn runtime_quota_window_summary_from_usage_snapshot_at(
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_at: i64,
    now: i64,
) -> RuntimeQuotaWindowSummary {
    runtime_quota_window_summary_from_proxy(
        runtime_proxy::runtime_proxy_quota_window_summary_from_usage_snapshot_at(
            runtime_quota_window_status_to_proxy(status),
            remaining_percent,
            reset_at,
            now,
        ),
    )
}

pub fn runtime_quota_window_usable_for_auto_rotate(status: RuntimeQuotaWindowStatus) -> bool {
    matches!(
        status,
        RuntimeQuotaWindowStatus::Ready
            | RuntimeQuotaWindowStatus::Thin
            | RuntimeQuotaWindowStatus::Critical
    )
}
