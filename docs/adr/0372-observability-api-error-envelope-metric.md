# ADR 0372: Observability API Error Envelope Metric

## Status

Accepted.

## Context

Enterprise API governance requires stable machine-readable error envelopes.
Operators need to count emitted, redacted, validation-failed, and
compatibility-rejected envelope outcomes without exposing raw error text,
provider secrets, response bodies, tenant IDs, or request IDs as metric labels.

## Decision

Add `plan_api_error_envelope_metric` to `prodex-observability`.

The planner emits `prodex_api_error_envelope_events_total`, increments by one,
and uses only the closed enum labels `api_error_envelope_surface` and
`api_error_envelope_result`.

## Consequences

- Gateway, control-plane, SCIM, and health adapters can publish stable error
  envelope outcomes through a shared low-cardinality contract.
- Raw error text, provider secrets, response bodies, tenants, and request IDs
  remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the error-envelope telemetry
  contract before a concrete metrics backend is wired in.
