# ADR 0754: Redact domain audit query-page error debug output

## Status

Accepted

## Context

Audit query pagination wraps query-plan, timestamp, and cursor validation
errors. `AuditQueryPageError` should preserve branch shape without relying on
derived `Debug`, because pagination can carry cursor positions and rejected
timestamps.

## Decision

Implement custom `Debug` for `AuditQueryPageError`. Preserve `Query`,
`Timestamp`, and `Cursor` branch names and delegate only to nested domain errors
with existing redaction contracts.

## Consequences

Diagnostics still identify the pagination failure branch. Raw timestamps,
cursor positions, tenant IDs, and audit event IDs remain out of formatter
output.
