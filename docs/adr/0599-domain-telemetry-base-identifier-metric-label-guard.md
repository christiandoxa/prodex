# ADR 0599: Domain Telemetry Base Identifier Metric Label Guard

## Status

Accepted.

## Context

The domain telemetry boundary rejects high-cardinality identifier labels such as
`tenant_id`, `request_id`, and camelCase variants. A raw identifier could still
be published under a base label key such as `tenant`, `user`, or `request`.

## Decision

Metric label key validation now rejects exact base identifier keys for
`tenant`, `user`, `principal`, `request`, `call`, and `prompt`. Closed
low-cardinality lifecycle labels such as `tenant_isolation_surface` remain
valid.

## Consequences

Adapters must publish raw identifiers as trace-only or redacted trace-only
attributes. Metric labels keep only closed categorical surfaces/results.
