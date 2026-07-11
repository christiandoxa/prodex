# ADR 0847: Redact domain observability span name debug output

Status: Accepted

## Context

`GatewaySpanDescriptor` carries span names for trace planning. Span names should
be low-cardinality, but request paths or operation metadata can accidentally
enter them before adapter boundaries normalize telemetry.

## Decision

Keep the stored span name unchanged, but redact `GatewaySpanDescriptor` span
names in `Debug` output alongside the existing attribute-list redaction.

## Consequences

Telemetry planning and serialization remain unchanged, while generic
diagnostics no longer expose span names.
