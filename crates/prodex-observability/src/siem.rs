use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiemOutboxHealthStatus {
    Healthy,
    Lagging,
    DeadLettered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiemOutboxHealthMetricPlan {
    pub pending_metric_name: &'static str,
    pub dead_letter_metric_name: &'static str,
    pub lag_metric_name: &'static str,
    pub pending: u64,
    pub dead_lettered: u64,
    pub lag_milliseconds: u64,
    pub status_label: TelemetryAttribute,
}

pub fn plan_siem_outbox_health_metric(
    pending: u64,
    dead_lettered: u64,
    oldest_pending_at_unix_ms: Option<u64>,
    observed_at_unix_ms: u64,
    maximum_lag_ms: u64,
) -> Result<SiemOutboxHealthMetricPlan, TelemetryAttributeError> {
    let lag_milliseconds = oldest_pending_at_unix_ms
        .map(|created_at| observed_at_unix_ms.saturating_sub(created_at))
        .unwrap_or(0);
    let status = if dead_lettered > 0 {
        SiemOutboxHealthStatus::DeadLettered
    } else if lag_milliseconds > maximum_lag_ms {
        SiemOutboxHealthStatus::Lagging
    } else {
        SiemOutboxHealthStatus::Healthy
    };
    let status_label = TelemetryAttribute::metric_label(
        "siem_outbox_status",
        match status {
            SiemOutboxHealthStatus::Healthy => "healthy",
            SiemOutboxHealthStatus::Lagging => "lagging",
            SiemOutboxHealthStatus::DeadLettered => "dead_lettered",
        },
    );
    status_label.as_metric_label()?;

    Ok(SiemOutboxHealthMetricPlan {
        pending_metric_name: "prodex_governance_siem_outbox_pending",
        dead_letter_metric_name: "prodex_governance_siem_outbox_dead_lettered",
        lag_metric_name: "prodex_governance_siem_outbox_oldest_pending_lag_milliseconds",
        pending,
        dead_lettered,
        lag_milliseconds,
        status_label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outbox_health_is_content_free_and_bounded_to_closed_status_labels() {
        let lagging = plan_siem_outbox_health_metric(3, 0, Some(1_000), 70_000, 60_000).unwrap();
        assert_eq!(lagging.pending, 3);
        assert_eq!(lagging.lag_milliseconds, 69_000);
        assert_eq!(
            lagging.status_label.as_metric_label().unwrap(),
            ("siem_outbox_status", "lagging")
        );
        let rendered = format!("{lagging:?}");
        for forbidden in ["tenant_id", "event_id", "payload", "digest", "endpoint"] {
            assert!(!rendered.contains(forbidden));
        }

        let dead = plan_siem_outbox_health_metric(0, 1, None, 70_000, 60_000).unwrap();
        assert_eq!(
            dead.status_label.as_metric_label().unwrap(),
            ("siem_outbox_status", "dead_lettered")
        );
    }

    #[test]
    fn outbox_lag_uses_saturating_clock_math() {
        let healthy = plan_siem_outbox_health_metric(1, 0, Some(2_000), 1_000, 60_000).unwrap();
        assert_eq!(healthy.lag_milliseconds, 0);
        assert_eq!(
            healthy.status_label.as_metric_label().unwrap(),
            ("siem_outbox_status", "healthy")
        );
    }
}
