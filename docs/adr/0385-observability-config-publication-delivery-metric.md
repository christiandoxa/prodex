# ADR 0385: Observability Configuration Publication Delivery Metric

## Status

Accepted.

## Context

Configuration revision publication must refresh gateway caches, reload runtime policy, and feed operational projections without hiding delivery failures. Operators need counters for target delivery outcomes without exposing tenant IDs, revision IDs, event topics, payloads, request IDs, caller identity, or raw delivery errors as metric labels.

## Decision

Add `plan_config_publication_delivery_metric` to `prodex-observability`.

The planner emits `prodex_config_publication_delivery_total`, increments by one, and uses only the closed enum labels `config_publication_target` and `config_publication_result`.

## Consequences

- Configuration publication adapters can publish gateway cache refresh, runtime policy reload, and audit projection delivery outcomes through one shared low-cardinality contract.
- Tenant IDs, revision IDs, topics, payloads, caller identity, and raw delivery errors remain trace/log/audit data subject to redaction.
- The observability boundary guard can enforce the configuration publication delivery telemetry contract before a concrete metrics backend is wired in.
