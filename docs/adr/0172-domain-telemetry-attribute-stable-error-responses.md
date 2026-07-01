# 0172: Domain Telemetry Attribute Stable Error Responses

## Status

Accepted

## Context

Enterprise observability requires RED metrics, trace propagation, bounded metric
labels, and redaction of tenant/user/key/prompt/high-cardinality values. Domain
telemetry validation rejects trace-only values used as metric labels, forbidden
metric label keys, and overlong label values.

Raw telemetry validation failures can include label keys, values, and length
details. These are useful in trusted diagnostics but must not be returned
directly through gateway or control-plane APIs.

## Decision

`prodex-domain` owns `plan_telemetry_attribute_error_response`.

The planner maps `TelemetryAttributeError` to a stable status/code/message
response plan. It deliberately omits metric label keys, values, lengths, tenant
IDs, user IDs, principal IDs, virtual-key identifiers, API keys, request/call
IDs, prompts, and other high-cardinality telemetry material.

## Consequences

- Composition roots can reject invalid telemetry planning without leaking
  observability internals.
- Metric-label guardrails remain enforceable in domain code.
- Raw telemetry errors remain available only to trusted logs and diagnostics.
