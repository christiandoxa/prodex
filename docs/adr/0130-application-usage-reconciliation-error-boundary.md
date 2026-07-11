# ADR 0130: Application usage reconciliation error boundary

## Status

Accepted

## Context

`prodex-application` composes post-provider usage reconciliation through gateway
accounting validation and durable PostgreSQL or SQLite storage planning. Raw
errors can include tenant identifiers, token/cost amounts, idempotency details,
backend names, or telemetry planning internals. Those details are useful for
trusted diagnostics but should not become client-visible API responses.

## Decision

Add `plan_application_usage_reconciliation_error_response` to
`prodex-application`. It maps invalid gateway reconciliation requests to
`usage_reconciliation_rejected`, storage planning failures to
`usage_reconciliation_storage_unavailable`, and telemetry planning failures to
`telemetry_unavailable`.

The response plan is HTTP-neutral and exposes only status, code, and a generic
message. Raw errors remain available for trusted redacted logs and audit-driven
reconciliation workflows.

## Consequences

Composition roots can report post-provider reconciliation failures without
leaking tenant IDs, usage amounts, SQL backend names, idempotency values, or
telemetry implementation details.
