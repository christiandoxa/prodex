# ADR 0829: Redact domain observability debug output

Status: Accepted

## Context

Domain observability descriptors carry trace-only attributes such as tenant IDs
and telemetry attribute values. Derived `Debug` output exposed those values and
metric-label validation lengths through diagnostics.

## Decision

Use custom `Debug` implementations for telemetry attributes, span descriptors,
and telemetry attribute errors that preserve shape while redacting values,
attribute lists, and rejected lengths.

## Consequences

Diagnostics can still distinguish span kind, attribute keys, scopes, and
validation failure variants, but raw telemetry values and rejected lengths no
longer appear through domain observability debug output.
