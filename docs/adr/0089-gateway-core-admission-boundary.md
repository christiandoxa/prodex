# ADR 0089: Gateway core admission boundary

## Status
Accepted

## Context
The target architecture separates a stateless gateway data plane from the
application composition root and future HTTP/runtime adapters. Data-plane request
admission must combine tenant-aware authorization, atomic reservation planning,
provider invocation validation, and telemetry span planning without importing
HTTP frameworks, database drivers, provider SDKs, filesystem access, or async
runtimes.

## Decision
Introduce `prodex-gateway-core` as an HTTP-neutral boundary crate. The crate
builds a gateway admission plan from canonical `Principal`, `TenantContext`, a
tenant-scoped resource, an atomic reservation command, a provider invocation, and
propagated trace context. Admission validates the data-plane authorization
boundary, rejects cross-tenant storage/provider context, plans the reservation,
validates provider invocation scope, and creates span plans for authorization,
budget reservation, and provider request stages.

The crate depends only on boundary/domain crates: `prodex-authz`,
`prodex-domain`, `prodex-observability`, `prodex-provider-spi`, and
`prodex-storage`.

## Consequences
Gateway HTTP/runtime code can migrate toward this core incrementally while
preserving existing proxy behavior. Concrete HTTP body handling, provider
transport, database transactions, and OpenTelemetry emission remain in adapter or
composition crates; this boundary keeps data-plane admission testable and free of
request-path side effects.
