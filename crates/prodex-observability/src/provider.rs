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

#[cfg(not(feature = "mojo"))]
mod rust_compat {
    use super::*;
    pub fn plan_provider_metric(
        provider: ProviderKind,
        result: ProviderResultClass,
        duration_ms: u64,
    ) -> Result<ProviderMetricPlan, TelemetryAttributeError> {
        let provider_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(103, "provider"),
            provider_kind_label(provider),
        )?;
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
        let provider_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(103, "provider"),
            provider_kind_label(provider),
        )?;
        let capability_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(104, "provider_capability"),
            provider_capability_kind_label(capability),
        )?;
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
        let provider_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(103, "provider"),
            provider_kind_label(provider),
        )?;
        let stage_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(114, "provider_retry_stage"),
            provider_retry_attempt_stage_label(stage),
        )?;
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
        let provider_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(103, "provider"),
            provider_kind_label(provider),
        )?;
        let decision_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(106, "provider_circuit_breaker_decision"),
            provider_circuit_breaker_decision_label(decision),
        )?;
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
        let provider_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(103, "provider"),
            provider_kind_label(provider),
        )?;
        let signal_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(111, "provider_degradation_signal"),
            provider_degradation_signal_label(signal),
        )?;
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
        let transport_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(141, "stream_transport"),
            stream_transport_kind_label(transport),
        )?;
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
        let lane_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(125, "routing_lane"),
            routing_lane_kind_label(lane),
        )?;
        let outcome_label = crate::planning_support::validated_metric_label(
            crate::planning_support::label_key(126, "routing_outcome"),
            routing_decision_outcome_label(outcome),
        )?;
        Ok(RoutingDecisionMetricPlan {
            metric_name: crate::planning_support::metric_name(
                52,
                0,
                "prodex_routing_decisions_total",
            ),
            increment: 1,
            lane_label,
            outcome_label,
        })
    }

    #[cfg(not(feature = "mojo"))]
    fn provider_kind_label(provider: ProviderKind) -> String {
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
}

#[cfg(not(feature = "mojo"))]
pub use rust_compat::*;

#[cfg(feature = "mojo")]
mod mojo_impl {
    use super::*;
    macro_rules! two_label_plan {
        ($name:ident, $return_type:ident, $plan:literal, $left:ident : $left_type:ty => $left_label:ident, $right:ident : $right_type:ty => $right_label:ident) => {
            pub fn $name(
                $left: $left_type,
                $right: $right_type,
            ) -> Result<$return_type, TelemetryAttributeError> {
                Ok($return_type {
                    metric_name: crate::planning_support::metric_name($plan, 0, ""),
                    increment: 1,
                    $left_label: crate::planning_support::planned_metric_label(
                        $plan,
                        0,
                        $left as i64,
                    )?,
                    $right_label: crate::planning_support::planned_metric_label(
                        $plan,
                        1,
                        $right as i64,
                    )?,
                })
            }
        };
    }
    macro_rules! three_label_plan {
        ($name:ident, $return_type:ident, $plan:literal, $a:ident : $ta:ty => $la:ident, $b:ident : $tb:ty => $lb:ident, $c:ident : $tc:ty => $lc:ident) => {
            pub fn $name(
                $a: $ta,
                $b: $tb,
                $c: $tc,
            ) -> Result<$return_type, TelemetryAttributeError> {
                Ok($return_type {
                    metric_name: crate::planning_support::metric_name($plan, 0, ""),
                    increment: 1,
                    $la: crate::planning_support::planned_metric_label($plan, 0, $a as i64)?,
                    $lb: crate::planning_support::planned_metric_label($plan, 1, $b as i64)?,
                    $lc: crate::planning_support::planned_metric_label($plan, 2, $c as i64)?,
                })
            }
        };
    }
    pub fn plan_provider_metric(
        provider: ProviderKind,
        result: ProviderResultClass,
        duration_ms: u64,
    ) -> Result<ProviderMetricPlan, TelemetryAttributeError> {
        Ok(ProviderMetricPlan {
            request_count_metric_name: crate::planning_support::metric_name(50, 0, ""),
            duration_metric_name: crate::planning_support::metric_name(50, 1, ""),
            increment: 1,
            duration_ms,
            provider_label: crate::planning_support::planned_metric_label(50, 0, provider as i64)?,
            result_label: crate::planning_support::planned_metric_label(50, 1, result as i64)?,
        })
    }
    pub fn plan_streaming_lifecycle_metric(
        transport: StreamTransportKind,
        outcome: StreamOutcome,
        duration_ms: u64,
    ) -> Result<StreamingLifecycleMetricPlan, TelemetryAttributeError> {
        Ok(StreamingLifecycleMetricPlan {
            event_count_metric_name: crate::planning_support::metric_name(53, 0, ""),
            duration_metric_name: crate::planning_support::metric_name(53, 1, ""),
            increment: 1,
            duration_ms,
            transport_label: crate::planning_support::planned_metric_label(
                53,
                0,
                transport as i64,
            )?,
            outcome_label: crate::planning_support::planned_metric_label(53, 1, outcome as i64)?,
        })
    }
    three_label_plan!(plan_provider_capability_negotiation_metric, ProviderCapabilityNegotiationMetricPlan, 47, provider: ProviderKind => provider_label, capability: ProviderCapabilityKind => capability_label, result: ProviderCapabilityResult => result_label);
    three_label_plan!(plan_provider_retry_metric, ProviderRetryMetricPlan, 51, provider: ProviderKind => provider_label, stage: ProviderRetryAttemptStage => stage_label, outcome: ProviderRetryOutcome => outcome_label);
    three_label_plan!(plan_provider_circuit_breaker_metric, ProviderCircuitBreakerMetricPlan, 48, provider: ProviderKind => provider_label, decision: ProviderCircuitBreakerDecision => decision_label, event: ProviderCircuitBreakerEvent => event_label);
    three_label_plan!(plan_provider_degradation_metric, ProviderDegradationMetricPlan, 49, provider: ProviderKind => provider_label, signal: ProviderDegradationSignal => signal_label, severity: ProviderDegradationSeverity => severity_label);
    two_label_plan!(plan_routing_decision_metric, RoutingDecisionMetricPlan, 52, lane: RoutingLaneKind => lane_label, outcome: RoutingDecisionOutcome => outcome_label);
}
#[cfg(feature = "mojo")]
pub use mojo_impl::*;
