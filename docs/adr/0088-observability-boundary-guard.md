# ADR 0088: Observability boundary guard

## Status
Accepted

## Context
`prodex-observability` is intended to be a backend-neutral boundary for trace
context, span planning, and metric label validation. It must not directly depend
on OpenTelemetry SDKs, tracing subscribers, Prometheus exporters, HTTP
frameworks, storage, filesystem logging, or async runtimes. Those belong in
adapter/composition crates.

## Decision
Add `scripts/ci/observability-boundary-guard.mjs` and wire it into npm scripts
and local preflight. The guard requires `prodex-observability` to depend only on
`prodex-domain`, forbids dev-dependencies and target-specific dependencies, and
scans source files for forbidden filesystem, environment, process, network,
HTTP, database, transport, OpenTelemetry SDK, Prometheus, tracing backend, and
async-runtime imports.

## Consequences
Observability policy remains deterministic and reusable while future adapters can
translate span plans into concrete OpenTelemetry/tracing/metrics backends without
leaking backend dependencies into the boundary contract.
