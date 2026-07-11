# ADR 0131: Application expired reservation recovery error boundary

## Status

Accepted

## Context

Expired reservation recovery is a background accounting use case that composes
gateway validation, durable PostgreSQL or SQLite recovery planning, and optional
Redis coordination leases. Raw errors can include tenant identifiers, usage
amounts, lease-owner details, SQL backend names, Redis implementation details,
or telemetry planning internals. Those details are useful for trusted operator
diagnostics but should not be returned to API clients or exposed through generic
composition-root response envelopes.

## Decision

Add `plan_application_expired_reservation_recovery_error_response` to
`prodex-application`. It maps invalid recovery requests to
`expired_reservation_recovery_rejected`, durable storage failures to
`expired_reservation_recovery_storage_unavailable`, Redis coordination failures
to `recovery_coordination_unavailable`, and telemetry planning failures to
`telemetry_unavailable`.

The helper is HTTP-neutral and returns only status, code, and a generic message.
Raw errors remain available to trusted, redacted diagnostics and recovery-worker
logs.

## Consequences

Recovery workers and composition roots can report recovery failures
consistently without leaking tenant IDs, usage amounts, backend names, Redis
lease owners, or telemetry internals.
