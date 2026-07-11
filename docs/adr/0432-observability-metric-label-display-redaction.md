# ADR 0432: Redact observability metric label display values

## Status

Accepted.

## Context

Span planning rejects high-cardinality metric labels, but its local `Display`
text used debug formatting for the underlying validation error. That could
include rejected metric keys or values.

## Decision

`SpanPlanError::MetricLabel` display text now reports only the error class.
The typed error still carries the structured validation error for callers that
intentionally inspect it.

## Consequences

Telemetry planning errors avoid echoing rejected metric labels into logs or API
adapter errors.
