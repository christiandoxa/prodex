# ADR 0600: Domain Telemetry UUID Metric Label Value Guard

## Status

Accepted.

## Context

The domain telemetry boundary rejects high-cardinality metric label keys, but a
raw tenant, principal, request, call, or audit identifier could still appear as
the value of an otherwise allowed label key.

## Decision

Metric label validation now rejects canonical UUID-shaped label values. The
stable client-visible error remains `telemetry_metric_label_forbidden`.

## Consequences

Raw globally unique identifiers must remain trace-only or redacted trace-only.
Closed categorical values such as provider names, routes, results, and status
classes remain valid metric labels.
