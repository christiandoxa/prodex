# ADR 0087: Observability boundary crate

## Status
Accepted

## Context
Prodex needs native trace propagation, span planning, metric labels, and log
correlation across gateway and control-plane flows. The domain crate already
models correlation context, gateway span kinds, and telemetry attribute
cardinality guardrails, but serving code needs a reusable boundary that applies
those rules before an OpenTelemetry SDK, HTTP framework, or logging backend is
introduced.

## Decision
Introduce `prodex-observability` as a boundary crate. It models W3C
`traceparent` parsing/rendering, span planning, and validation of low-cardinality
metric labels. A span plan must have either propagated trace context or a
correlation trace ID, and metric labels are validated through the domain
cardinality rules so raw tenant/principal/request/call identifiers remain trace
only.

The crate intentionally depends only on `prodex-domain` and does not import an
OpenTelemetry SDK, HTTP middleware, filesystem logging, storage, or async
runtime.

## Consequences
Gateway/control-plane adapters can translate `SpanPlan` into OpenTelemetry spans
in later adapter crates while keeping cardinality and propagation rules shared
and deterministic. This also provides a migration path from custom telemetry to
standards-based tracing without coupling domain or boundary crates to a concrete
telemetry backend.
