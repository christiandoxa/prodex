# ADR 0141: Application expired recovery gateway error boundary

## Status

Accepted

## Context

`prodex-application` composes durable expired-reservation recovery storage,
optional Redis recovery-lease coordination, and gateway recovery planning. ADR
0131 introduced a stable application-level recovery response planner, and ADR
0138 introduced the gateway-core response planner for tenant-affinity,
recovery-command, and telemetry failures. Keeping a second gateway mapping in
the application layer can drift from the data-plane gateway contract.

## Decision

`plan_application_expired_reservation_recovery_error_response` now delegates
gateway recovery failures to
`plan_gateway_expired_reservation_recovery_error_response` and adapts only the
status enum into the application response type.

Redis coordination and durable storage planning failures remain
application-owned and continue to use the stable
`recovery_coordination_unavailable` and
`expired_reservation_recovery_storage_unavailable` responses.

## Consequences

Application composition roots share the same redacted gateway response envelope
for expired-reservation recovery failures while still hiding Redis, PostgreSQL,
and SQLite planning internals behind application-owned coordination and storage
boundaries.
