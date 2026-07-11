# ADR 0140: Application usage reconciliation gateway error boundary

## Status

Accepted

## Context

`prodex-application` composes durable accounting plans with gateway usage
reconciliation. ADR 0130 introduced a stable application-level response planner,
and ADR 0137 introduced the gateway-core planner for tenant-affinity,
reconciliation, and telemetry failures. Keeping an independent copy of the
gateway mapping in the application layer risks response drift as gateway
reconciliation evolves.

## Decision

`plan_application_usage_reconciliation_error_response` now delegates gateway
usage reconciliation failures to
`plan_gateway_usage_reconciliation_error_response` and adapts only the status
enum into the application response type.

Durable storage planning failures remain application-owned and continue to use
the stable `usage_reconciliation_storage_unavailable` response.

## Consequences

Application composition roots share the same redacted gateway response envelope
for post-provider reconciliation failures while still hiding PostgreSQL and
SQLite planning internals behind a storage-specific application boundary.
