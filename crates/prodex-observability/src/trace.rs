use std::error::Error;
use std::fmt;

use prodex_domain::{
    CorrelationContext, GatewaySpanDescriptor, GatewaySpanKind, TelemetryAttribute,
    TelemetryAttributeError, TraceId, TraceIdError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: String,
    pub trace_flags: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceContextError {
    InvalidTraceId(TraceIdError),
    InvalidSpanId,
    InvalidFlags,
    MalformedTraceparent,
    InvalidTracestate,
    InvalidBaggage,
}

impl fmt::Display for TraceContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTraceId(err) => err.fmt(f),
            Self::InvalidSpanId => write!(f, "trace context is invalid"),
            Self::InvalidFlags => write!(f, "trace context is invalid"),
            Self::MalformedTraceparent => write!(f, "trace context is invalid"),
            Self::InvalidTracestate => write!(f, "trace context is invalid"),
            Self::InvalidBaggage => write!(f, "trace context is invalid"),
        }
    }
}

impl Error for TraceContextError {}

impl TraceContext {
    pub fn new(
        trace_id: impl Into<String>,
        span_id: impl Into<String>,
        trace_flags: impl Into<String>,
    ) -> Result<Self, TraceContextError> {
        let trace_id = TraceId::new(trace_id).map_err(TraceContextError::InvalidTraceId)?;
        let span_id = span_id.into().trim().to_ascii_lowercase();
        if span_id.len() != 16 || !span_id.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(TraceContextError::InvalidSpanId);
        }
        let trace_flags = trace_flags.into().trim().to_ascii_lowercase();
        if trace_flags.len() != 2 || !trace_flags.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Err(TraceContextError::InvalidFlags);
        }
        Ok(Self {
            trace_id,
            span_id,
            trace_flags,
        })
    }

    pub fn parse_traceparent(value: &str) -> Result<Self, TraceContextError> {
        let parts = value.trim().split('-').collect::<Vec<_>>();
        if parts.len() != 4 || parts[0] != "00" {
            return Err(TraceContextError::MalformedTraceparent);
        }
        Self::new(parts[1], parts[2], parts[3])
    }

    pub fn traceparent(&self) -> String {
        format!(
            "00-{}-{}-{}",
            self.trace_id.as_str(),
            self.span_id,
            self.trace_flags
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TracePropagationPlan {
    pub traceparent: String,
    pub tracestate: Option<String>,
    pub baggage: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TracePropagationCarrier {
    Traceparent,
    Tracestate,
    Baggage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TracePropagationResult {
    Propagated,
    Rejected,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TracePropagationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub carrier_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

pub fn plan_trace_propagation(
    trace_context: &TraceContext,
    tracestate: Option<&str>,
    baggage: Option<&str>,
) -> Result<TracePropagationPlan, TraceContextError> {
    let tracestate = tracestate.map(validate_tracestate).transpose()?;
    let baggage = baggage.map(validate_baggage).transpose()?;
    Ok(TracePropagationPlan {
        traceparent: trace_context.traceparent(),
        tracestate,
        baggage,
    })
}

fn validate_tracestate(value: &str) -> Result<String, TraceContextError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 512
        || value
            .chars()
            .any(|character| !character.is_ascii_graphic() && character != ' ')
    {
        return Err(TraceContextError::InvalidTracestate);
    }
    Ok(value.to_string())
}

fn validate_baggage(value: &str) -> Result<String, TraceContextError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 8192
        || value
            .chars()
            .any(|character| !character.is_ascii_graphic() && character != ' ')
    {
        return Err(TraceContextError::InvalidBaggage);
    }
    Ok(value.to_string())
}

pub fn plan_trace_propagation_metric(
    carrier: TracePropagationCarrier,
    result: TracePropagationResult,
) -> Result<TracePropagationMetricPlan, TelemetryAttributeError> {
    let carrier_label =
        TelemetryAttribute::metric_label("trace_carrier", trace_propagation_carrier_label(carrier));
    let result_label = TelemetryAttribute::metric_label(
        "trace_propagation_result",
        trace_propagation_result_label(result),
    );
    carrier_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(TracePropagationMetricPlan {
        metric_name: "prodex_trace_propagation_events_total",
        increment: 1,
        carrier_label,
        result_label,
    })
}

fn trace_propagation_carrier_label(carrier: TracePropagationCarrier) -> &'static str {
    match carrier {
        TracePropagationCarrier::Traceparent => "traceparent",
        TracePropagationCarrier::Tracestate => "tracestate",
        TracePropagationCarrier::Baggage => "baggage",
    }
}

fn trace_propagation_result_label(result: TracePropagationResult) -> &'static str {
    match result {
        TracePropagationResult::Propagated => "propagated",
        TracePropagationResult::Rejected => "rejected",
        TracePropagationResult::Missing => "missing",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanPlan {
    pub descriptor: GatewaySpanDescriptor,
    pub trace_context: Option<TraceContext>,
    pub correlation: CorrelationContext,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuredLogCorrelationPlan {
    pub fields: Vec<TelemetryAttribute>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanPlanError {
    MissingCorrelationTrace,
    MetricLabel(TelemetryAttributeError),
}

impl fmt::Display for SpanPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCorrelationTrace => write!(f, "span requires propagated trace context"),
            Self::MetricLabel(_) => write!(f, "invalid metric label"),
        }
    }
}

impl Error for SpanPlanError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservabilityErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservabilityErrorResponsePlan {
    pub status: ObservabilityErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_span_error_response(_error: &SpanPlanError) -> ObservabilityErrorResponsePlan {
    ObservabilityErrorResponsePlan {
        status: ObservabilityErrorStatus::ServiceUnavailable,
        code: "telemetry_unavailable",
        message: "telemetry planning is temporarily unavailable",
    }
}

pub fn plan_trace_context_error_response(
    _error: &TraceContextError,
) -> ObservabilityErrorResponsePlan {
    ObservabilityErrorResponsePlan {
        status: ObservabilityErrorStatus::ServiceUnavailable,
        code: "invalid_trace_context",
        message: "trace context is required and must be valid",
    }
}

pub fn plan_gateway_span(
    kind: GatewaySpanKind,
    name: impl Into<String>,
    correlation: CorrelationContext,
    trace_context: Option<TraceContext>,
    attributes: Vec<TelemetryAttribute>,
) -> Result<SpanPlan, SpanPlanError> {
    if correlation.trace_id.is_none() && trace_context.is_none() {
        return Err(SpanPlanError::MissingCorrelationTrace);
    }
    let mut descriptor = GatewaySpanDescriptor::new(kind, name);
    for attribute in attributes {
        if attribute.scope == prodex_domain::TelemetryAttributeScope::MetricLabel {
            attribute
                .as_metric_label()
                .map_err(SpanPlanError::MetricLabel)?;
        }
        descriptor = descriptor.with_attribute(attribute);
    }
    Ok(SpanPlan {
        descriptor,
        trace_context,
        correlation,
    })
}

pub fn plan_structured_log_correlation(
    correlation: &CorrelationContext,
) -> StructuredLogCorrelationPlan {
    let mut fields = vec![TelemetryAttribute::trace_only(
        "request_id",
        correlation.request_id.to_string(),
    )];
    if let Some(call_id) = correlation.call_id {
        fields.push(TelemetryAttribute::trace_only(
            "call_id",
            call_id.to_string(),
        ));
    }
    if let Some(trace_id) = &correlation.trace_id {
        fields.push(TelemetryAttribute::trace_only(
            "trace_id",
            trace_id.as_str(),
        ));
    }
    if let Some(tenant_id) = correlation.tenant_id {
        fields.push(TelemetryAttribute::trace_only(
            "tenant_id",
            tenant_id.to_string(),
        ));
    }
    if let Some(audit_event_id) = correlation.audit_event_id {
        fields.push(TelemetryAttribute::trace_only(
            "audit_event_id",
            audit_event_id.to_string(),
        ));
    }
    StructuredLogCorrelationPlan { fields }
}
