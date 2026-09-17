use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
    Gemini,
    Local,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderResultClass {
    Success,
    RateLimited,
    Overloaded,
    ProviderError,
    TransportError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderMetricPlan {
    pub request_count_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_ms: u64,
    pub provider_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCapabilityKind {
    ResponsesApi,
    Streaming,
    Tools,
    Vision,
    JsonMode,
    RemoteCompact,
    WebSocket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCapabilityResult {
    Compatible,
    Incompatible,
    NoCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCapabilityNegotiationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub provider_label: TelemetryAttribute,
    pub capability_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryAttemptStage {
    BeforeDispatch,
    BeforeFirstByte,
    AfterFirstByte,
    AfterCancellation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderRetryOutcome {
    Allowed,
    DeniedCommitted,
    DeniedBudgetExhausted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRetryMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub provider_label: TelemetryAttribute,
    pub stage_label: TelemetryAttribute,
    pub outcome_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCircuitBreakerDecision {
    Closed,
    Open,
    HalfOpenProbe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderCircuitBreakerEvent {
    Success,
    Failure,
    Probe,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderCircuitBreakerMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub provider_label: TelemetryAttribute,
    pub decision_label: TelemetryAttribute,
    pub event_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderDegradationSignal {
    ErrorRate,
    Latency,
    Overload,
    Transport,
    CircuitOpen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderDegradationSeverity {
    Warning,
    Critical,
    Recovered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderDegradationMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub provider_label: TelemetryAttribute,
    pub signal_label: TelemetryAttribute,
    pub severity_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamTransportKind {
    Responses,
    Websocket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamOutcome {
    Completed,
    Cancelled,
    Interrupted,
    GuardrailBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamingLifecycleMetricPlan {
    pub event_count_metric_name: &'static str,
    pub duration_metric_name: &'static str,
    pub increment: u64,
    pub duration_ms: u64,
    pub transport_label: TelemetryAttribute,
    pub outcome_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingLaneKind {
    Responses,
    Compact,
    Websocket,
    ControlPlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingDecisionOutcome {
    Selected,
    Fallback,
    Rejected,
    NoCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingDecisionMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub lane_label: TelemetryAttribute,
    pub outcome_label: TelemetryAttribute,
}

pub fn plan_provider_metric(
    provider: ProviderKind,
    result: ProviderResultClass,
    duration_ms: u64,
) -> Result<ProviderMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let provider_label = crate::planning_support::planned_metric_label(50, 0, (provider) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let provider_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(103, "provider"),
        provider_kind_label(provider),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(50, 1, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(112, "provider_result"),
        provider_result_class_label(result),
    )?;
    Ok(ProviderMetricPlan {
        request_count_metric_name: crate::planning_support::metric_name(
            50,
            0,
            "prodex_provider_requests_total",
        ),
        duration_metric_name: crate::planning_support::metric_name(
            50,
            1,
            "prodex_provider_request_duration_ms",
        ),
        increment: 1,
        duration_ms,
        provider_label,
        result_label,
    })
}

pub fn plan_provider_capability_negotiation_metric(
    provider: ProviderKind,
    capability: ProviderCapabilityKind,
    result: ProviderCapabilityResult,
) -> Result<ProviderCapabilityNegotiationMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let provider_label = crate::planning_support::planned_metric_label(47, 0, (provider) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let provider_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(103, "provider"),
        provider_kind_label(provider),
    )?;
    #[cfg(feature = "mojo")]
    let capability_label =
        crate::planning_support::planned_metric_label(47, 1, (capability) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let capability_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(104, "provider_capability"),
        provider_capability_kind_label(capability),
    )?;
    #[cfg(feature = "mojo")]
    let result_label = crate::planning_support::planned_metric_label(47, 2, (result) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(105, "provider_capability_result"),
        provider_capability_result_label(result),
    )?;
    Ok(ProviderCapabilityNegotiationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            47,
            0,
            "prodex_provider_capability_negotiation_events_total",
        ),
        increment: 1,
        provider_label,
        capability_label,
        result_label,
    })
}

pub fn plan_provider_retry_metric(
    provider: ProviderKind,
    stage: ProviderRetryAttemptStage,
    outcome: ProviderRetryOutcome,
) -> Result<ProviderRetryMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let provider_label = crate::planning_support::planned_metric_label(51, 0, (provider) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let provider_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(103, "provider"),
        provider_kind_label(provider),
    )?;
    #[cfg(feature = "mojo")]
    let stage_label = crate::planning_support::planned_metric_label(51, 1, (stage) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let stage_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(114, "provider_retry_stage"),
        provider_retry_attempt_stage_label(stage),
    )?;
    #[cfg(feature = "mojo")]
    let outcome_label = crate::planning_support::planned_metric_label(51, 2, (outcome) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let outcome_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(113, "provider_retry_outcome"),
        provider_retry_outcome_label(outcome),
    )?;
    Ok(ProviderRetryMetricPlan {
        metric_name: crate::planning_support::metric_name(
            51,
            0,
            "prodex_provider_retry_events_total",
        ),
        increment: 1,
        provider_label,
        stage_label,
        outcome_label,
    })
}

pub fn plan_provider_circuit_breaker_metric(
    provider: ProviderKind,
    decision: ProviderCircuitBreakerDecision,
    event: ProviderCircuitBreakerEvent,
) -> Result<ProviderCircuitBreakerMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let provider_label = crate::planning_support::planned_metric_label(48, 0, (provider) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let provider_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(103, "provider"),
        provider_kind_label(provider),
    )?;
    #[cfg(feature = "mojo")]
    let decision_label = crate::planning_support::planned_metric_label(48, 1, (decision) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let decision_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(106, "provider_circuit_breaker_decision"),
        provider_circuit_breaker_decision_label(decision),
    )?;
    #[cfg(feature = "mojo")]
    let event_label = crate::planning_support::planned_metric_label(48, 2, (event) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let event_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(107, "provider_circuit_breaker_event"),
        provider_circuit_breaker_event_label(event),
    )?;
    Ok(ProviderCircuitBreakerMetricPlan {
        metric_name: crate::planning_support::metric_name(
            48,
            0,
            "prodex_provider_circuit_breaker_events_total",
        ),
        increment: 1,
        provider_label,
        decision_label,
        event_label,
    })
}

pub fn plan_provider_degradation_metric(
    provider: ProviderKind,
    signal: ProviderDegradationSignal,
    severity: ProviderDegradationSeverity,
) -> Result<ProviderDegradationMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let provider_label = crate::planning_support::planned_metric_label(49, 0, (provider) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let provider_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(103, "provider"),
        provider_kind_label(provider),
    )?;
    #[cfg(feature = "mojo")]
    let signal_label = crate::planning_support::planned_metric_label(49, 1, (signal) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let signal_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(111, "provider_degradation_signal"),
        provider_degradation_signal_label(signal),
    )?;
    #[cfg(feature = "mojo")]
    let severity_label = crate::planning_support::planned_metric_label(49, 2, (severity) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let severity_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(110, "provider_degradation_severity"),
        provider_degradation_severity_label(severity),
    )?;
    Ok(ProviderDegradationMetricPlan {
        metric_name: crate::planning_support::metric_name(
            49,
            0,
            "prodex_provider_degradation_events_total",
        ),
        increment: 1,
        provider_label,
        signal_label,
        severity_label,
    })
}

pub fn plan_streaming_lifecycle_metric(
    transport: StreamTransportKind,
    outcome: StreamOutcome,
    duration_ms: u64,
) -> Result<StreamingLifecycleMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let transport_label = crate::planning_support::planned_metric_label(53, 0, (transport) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let transport_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(141, "stream_transport"),
        stream_transport_kind_label(transport),
    )?;
    #[cfg(feature = "mojo")]
    let outcome_label = crate::planning_support::planned_metric_label(53, 1, (outcome) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let outcome_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(140, "stream_outcome"),
        stream_outcome_label(outcome),
    )?;
    Ok(StreamingLifecycleMetricPlan {
        event_count_metric_name: crate::planning_support::metric_name(
            53,
            0,
            "prodex_streaming_lifecycle_total",
        ),
        duration_metric_name: crate::planning_support::metric_name(
            53,
            1,
            "prodex_streaming_lifecycle_duration_ms",
        ),
        increment: 1,
        duration_ms,
        transport_label,
        outcome_label,
    })
}

pub fn plan_routing_decision_metric(
    lane: RoutingLaneKind,
    outcome: RoutingDecisionOutcome,
) -> Result<RoutingDecisionMetricPlan, TelemetryAttributeError> {
    #[cfg(feature = "mojo")]
    let lane_label = crate::planning_support::planned_metric_label(52, 0, (lane) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let lane_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(125, "routing_lane"),
        routing_lane_kind_label(lane),
    )?;
    #[cfg(feature = "mojo")]
    let outcome_label = crate::planning_support::planned_metric_label(52, 1, (outcome) as i64)?;
    #[cfg(not(feature = "mojo"))]
    let outcome_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(126, "routing_outcome"),
        routing_decision_outcome_label(outcome),
    )?;
    Ok(RoutingDecisionMetricPlan {
        metric_name: crate::planning_support::metric_name(52, 0, "prodex_routing_decisions_total"),
        increment: 1,
        lane_label,
        outcome_label,
    })
}

#[cfg(not(feature = "mojo"))]
fn provider_kind_label(provider: ProviderKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(94, provider as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match provider {
            ProviderKind::OpenAi => "openai",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Gemini => "gemini",
            ProviderKind::Local => "local",
            ProviderKind::Other => "other",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_result_class_label(result: ProviderResultClass) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(95, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ProviderResultClass::Success => "success",
            ProviderResultClass::RateLimited => "rate_limited",
            ProviderResultClass::Overloaded => "overloaded",
            ProviderResultClass::ProviderError => "provider_error",
            ProviderResultClass::TransportError => "transport_error",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_capability_kind_label(capability: ProviderCapabilityKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(89, capability as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match capability {
            ProviderCapabilityKind::ResponsesApi => "responses_api",
            ProviderCapabilityKind::Streaming => "streaming",
            ProviderCapabilityKind::Tools => "tools",
            ProviderCapabilityKind::Vision => "vision",
            ProviderCapabilityKind::JsonMode => "json_mode",
            ProviderCapabilityKind::RemoteCompact => "remote_compact",
            ProviderCapabilityKind::WebSocket => "websocket",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_capability_result_label(result: ProviderCapabilityResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(90, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            ProviderCapabilityResult::Compatible => "compatible",
            ProviderCapabilityResult::Incompatible => "incompatible",
            ProviderCapabilityResult::NoCandidate => "no_candidate",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_retry_attempt_stage_label(stage: ProviderRetryAttemptStage) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(96, stage as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match stage {
            ProviderRetryAttemptStage::BeforeDispatch => "before_dispatch",
            ProviderRetryAttemptStage::BeforeFirstByte => "before_first_byte",
            ProviderRetryAttemptStage::AfterFirstByte => "after_first_byte",
            ProviderRetryAttemptStage::AfterCancellation => "after_cancellation",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_retry_outcome_label(outcome: ProviderRetryOutcome) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(97, outcome as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match outcome {
            ProviderRetryOutcome::Allowed => "allowed",
            ProviderRetryOutcome::DeniedCommitted => "denied_committed",
            ProviderRetryOutcome::DeniedBudgetExhausted => "denied_budget_exhausted",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_circuit_breaker_decision_label(decision: ProviderCircuitBreakerDecision) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(137, decision as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match decision {
            ProviderCircuitBreakerDecision::Closed => "closed",
            ProviderCircuitBreakerDecision::Open => "open",
            ProviderCircuitBreakerDecision::HalfOpenProbe => "half_open_probe",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_circuit_breaker_event_label(event: ProviderCircuitBreakerEvent) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(91, event as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match event {
            ProviderCircuitBreakerEvent::Success => "success",
            ProviderCircuitBreakerEvent::Failure => "failure",
            ProviderCircuitBreakerEvent::Probe => "probe",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_degradation_signal_label(signal: ProviderDegradationSignal) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(93, signal as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match signal {
            ProviderDegradationSignal::ErrorRate => "error_rate",
            ProviderDegradationSignal::Latency => "latency",
            ProviderDegradationSignal::Overload => "overload",
            ProviderDegradationSignal::Transport => "transport",
            ProviderDegradationSignal::CircuitOpen => "circuit_open",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn provider_degradation_severity_label(severity: ProviderDegradationSeverity) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(92, severity as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match severity {
            ProviderDegradationSeverity::Warning => "warning",
            ProviderDegradationSeverity::Critical => "critical",
            ProviderDegradationSeverity::Recovered => "recovered",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn stream_transport_kind_label(transport: StreamTransportKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(101, transport as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match transport {
            StreamTransportKind::Responses => "responses",
            StreamTransportKind::Websocket => "websocket",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn stream_outcome_label(outcome: StreamOutcome) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(100, outcome as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match outcome {
            StreamOutcome::Completed => "completed",
            StreamOutcome::Cancelled => "cancelled",
            StreamOutcome::Interrupted => "interrupted",
            StreamOutcome::GuardrailBlocked => "guardrail_blocked",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn routing_lane_kind_label(lane: RoutingLaneKind) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(99, lane as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match lane {
            RoutingLaneKind::Responses => "responses",
            RoutingLaneKind::Compact => "compact",
            RoutingLaneKind::Websocket => "websocket",
            RoutingLaneKind::ControlPlane => "control_plane",
        })
        .to_string()
    }
}

#[cfg(not(feature = "mojo"))]
fn routing_decision_outcome_label(outcome: RoutingDecisionOutcome) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(98, outcome as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match outcome {
            RoutingDecisionOutcome::Selected => "selected",
            RoutingDecisionOutcome::Fallback => "fallback",
            RoutingDecisionOutcome::Rejected => "rejected",
            RoutingDecisionOutcome::NoCandidate => "no_candidate",
        })
        .to_string()
    }
}
