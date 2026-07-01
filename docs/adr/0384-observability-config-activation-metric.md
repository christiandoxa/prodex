# ADR 0384: Observability Configuration Activation Metric

## Status

Accepted.

## Context

Enterprise gateways need telemetry for configuration revision activation, last-known-good fallback, rollback, and invalidation fallback. Operators need to count activation outcomes without exposing tenant IDs, revision IDs, digests, configuration bodies, signatures, request IDs, caller identity, or raw validation errors as metric labels.

## Decision

Add `plan_config_activation_metric` to `prodex-observability`.

The planner emits `prodex_config_activation_events_total`, increments by one, and uses only the closed enum labels `config_activation_source` and `config_activation_result`.

## Consequences

- Configuration activation adapters can publish revision activation, last-known-good fallback, rollback, and invalidation fallback outcomes through one shared low-cardinality contract.
- Tenant IDs, revision IDs, digests, configuration payloads, signatures, caller identity, and raw validation errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the configuration activation telemetry contract before a concrete metrics backend is wired in.
