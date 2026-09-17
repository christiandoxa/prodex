use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetryDropReason {
    QueueFull,
    ExporterUnavailable,
    Shutdown,
    InvalidPayload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DroppedTelemetryMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub reason_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueDepthKind {
    Responses,
    Compact,
    Websocket,
    Telemetry,
    Persistence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueDepthMetricPlan {
    pub metric_name: &'static str,
    pub depth: u64,
    pub capacity: u64,
    pub queue_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionPoolKind {
    Postgres,
    Redis,
    ProviderHttp,
    OidcHttp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionPoolSaturationMetricPlan {
    pub metric_name: &'static str,
    pub in_use: u64,
    pub capacity: u64,
    pub pool_label: TelemetryAttribute,
}

#[cfg(not(feature = "mojo"))]
mod rust_compat {
    use super::*;
    pub fn plan_dropped_telemetry_metric(
        reason: TelemetryDropReason,
    ) -> Result<DroppedTelemetryMetricPlan, TelemetryAttributeError> {
        let reason_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(142, "telemetry_drop_reason"),
            telemetry_drop_reason_label(reason),
        )?;
        Ok(DroppedTelemetryMetricPlan {
            metric_name: crate::planning_support::metric_name(
                55,
                0,
                "prodex_telemetry_dropped_total",
            ),
            increment: 1,
            reason_label,
        })
    }

    pub fn plan_queue_depth_metric(
        kind: QueueDepthKind,
        depth: u64,
        capacity: u64,
    ) -> Result<QueueDepthMetricPlan, TelemetryAttributeError> {
        let queue_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(115, "queue_kind"),
            queue_depth_kind_label(kind),
        )?;
        Ok(QueueDepthMetricPlan {
            metric_name: crate::planning_support::metric_name(56, 0, "prodex_queue_depth"),
            depth,
            capacity,
            queue_label,
        })
    }

    pub fn plan_connection_pool_saturation_metric(
        kind: ConnectionPoolKind,
        in_use: u64,
        capacity: u64,
    ) -> Result<ConnectionPoolSaturationMetricPlan, TelemetryAttributeError> {
        let pool_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(100, "pool_kind"),
            connection_pool_kind_label(kind),
        )?;
        Ok(ConnectionPoolSaturationMetricPlan {
            metric_name: crate::planning_support::metric_name(
                54,
                0,
                "prodex_connection_pool_in_use",
            ),
            in_use,
            capacity,
            pool_label,
        })
    }

    #[cfg(not(feature = "mojo"))]
    fn telemetry_drop_reason_label(reason: TelemetryDropReason) -> String {
        {
            (match reason {
                TelemetryDropReason::QueueFull => "queue_full",
                TelemetryDropReason::ExporterUnavailable => "exporter_unavailable",
                TelemetryDropReason::Shutdown => "shutdown",
                TelemetryDropReason::InvalidPayload => "invalid_payload",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn queue_depth_kind_label(kind: QueueDepthKind) -> String {
        {
            (match kind {
                QueueDepthKind::Responses => "responses",
                QueueDepthKind::Compact => "compact",
                QueueDepthKind::Websocket => "websocket",
                QueueDepthKind::Telemetry => "telemetry",
                QueueDepthKind::Persistence => "persistence",
            })
            .to_string()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn connection_pool_kind_label(kind: ConnectionPoolKind) -> String {
        {
            (match kind {
                ConnectionPoolKind::Postgres => "postgres",
                ConnectionPoolKind::Redis => "redis",
                ConnectionPoolKind::ProviderHttp => "provider_http",
                ConnectionPoolKind::OidcHttp => "oidc_http",
            })
            .to_string()
        }
    }
}

#[cfg(not(feature = "mojo"))]
pub use rust_compat::*;

#[cfg(feature = "mojo")]
mod mojo_impl {
    use super::*;
    pub fn plan_dropped_telemetry_metric(
        reason: TelemetryDropReason,
    ) -> Result<DroppedTelemetryMetricPlan, TelemetryAttributeError> {
        Ok(DroppedTelemetryMetricPlan {
            metric_name: crate::planning_support::metric_name(55, 0, ""),
            increment: 1,
            reason_label: crate::planning_support::planned_metric_label(55, 0, reason as i64)?,
        })
    }
    pub fn plan_queue_depth_metric(
        kind: QueueDepthKind,
        depth: u64,
        capacity: u64,
    ) -> Result<QueueDepthMetricPlan, TelemetryAttributeError> {
        Ok(QueueDepthMetricPlan {
            metric_name: crate::planning_support::metric_name(56, 0, ""),
            depth,
            capacity,
            queue_label: crate::planning_support::planned_metric_label(56, 0, kind as i64)?,
        })
    }
    pub fn plan_connection_pool_saturation_metric(
        kind: ConnectionPoolKind,
        in_use: u64,
        capacity: u64,
    ) -> Result<ConnectionPoolSaturationMetricPlan, TelemetryAttributeError> {
        Ok(ConnectionPoolSaturationMetricPlan {
            metric_name: crate::planning_support::metric_name(54, 0, ""),
            in_use,
            capacity,
            pool_label: crate::planning_support::planned_metric_label(54, 0, kind as i64)?,
        })
    }
}
#[cfg(feature = "mojo")]
pub use mojo_impl::*;
