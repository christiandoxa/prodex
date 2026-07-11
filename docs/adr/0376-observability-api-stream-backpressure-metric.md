# ADR 0376: Observability API Stream Backpressure Metric

## Status

Accepted.

## Context

Enterprise HTTP data-plane hardening requires streaming backpressure for data-plane streams, provider streams, WebSocket transport, and audit export. Operators need to count ready, paused, dropped, and closed stream backpressure states without exposing stream IDs, session IDs, connection IDs, tenant IDs, or request IDs as metric labels.

## Decision

Add `plan_api_stream_backpressure_metric` to `prodex-observability`.

The planner emits `prodex_api_stream_backpressure_events_total`, increments by one, and uses only the closed enum labels `api_stream_backpressure_surface` and `api_stream_backpressure_state`.

## Consequences

- Gateway and control-plane streaming adapters can publish backpressure outcomes through a shared low-cardinality contract.
- Stream IDs, session IDs, connection IDs, tenants, and request IDs remain trace/log/report data subject to redaction.
- The observability boundary guard can enforce the stream-backpressure telemetry contract before a concrete metrics backend is wired in.
