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

pub fn plan_dropped_telemetry_metric(
    reason: TelemetryDropReason,
) -> Result<DroppedTelemetryMetricPlan, TelemetryAttributeError> {
    let reason_label = TelemetryAttribute::metric_label(
        "telemetry_drop_reason",
        telemetry_drop_reason_label(reason),
    );
    reason_label.as_metric_label()?;
    Ok(DroppedTelemetryMetricPlan {
        metric_name: "prodex_telemetry_dropped_total",
        increment: 1,
        reason_label,
    })
}

pub fn plan_queue_depth_metric(
    kind: QueueDepthKind,
    depth: u64,
    capacity: u64,
) -> Result<QueueDepthMetricPlan, TelemetryAttributeError> {
    let queue_label = TelemetryAttribute::metric_label("queue_kind", queue_depth_kind_label(kind));
    queue_label.as_metric_label()?;
    Ok(QueueDepthMetricPlan {
        metric_name: "prodex_queue_depth",
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
    let pool_label =
        TelemetryAttribute::metric_label("pool_kind", connection_pool_kind_label(kind));
    pool_label.as_metric_label()?;
    Ok(ConnectionPoolSaturationMetricPlan {
        metric_name: "prodex_connection_pool_in_use",
        in_use,
        capacity,
        pool_label,
    })
}

fn telemetry_drop_reason_label(reason: TelemetryDropReason) -> &'static str {
    match reason {
        TelemetryDropReason::QueueFull => "queue_full",
        TelemetryDropReason::ExporterUnavailable => "exporter_unavailable",
        TelemetryDropReason::Shutdown => "shutdown",
        TelemetryDropReason::InvalidPayload => "invalid_payload",
    }
}

fn queue_depth_kind_label(kind: QueueDepthKind) -> &'static str {
    match kind {
        QueueDepthKind::Responses => "responses",
        QueueDepthKind::Compact => "compact",
        QueueDepthKind::Websocket => "websocket",
        QueueDepthKind::Telemetry => "telemetry",
        QueueDepthKind::Persistence => "persistence",
    }
}

fn connection_pool_kind_label(kind: ConnectionPoolKind) -> &'static str {
    match kind {
        ConnectionPoolKind::Postgres => "postgres",
        ConnectionPoolKind::Redis => "redis",
        ConnectionPoolKind::ProviderHttp => "provider_http",
        ConnectionPoolKind::OidcHttp => "oidc_http",
    }
}
