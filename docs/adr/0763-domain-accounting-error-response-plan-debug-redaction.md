# ADR 0763: Redact domain accounting error-response plan debug output

## Status

Accepted

## Context

`AccountingErrorResponsePlan` is the domain boundary for budget reservation,
reservation commit, recovery, and reconciliation client-visible errors. It is
low-cardinality today, but future fields could carry tenant IDs, call IDs,
reservation IDs, usage amounts, or ledger details.

## Decision

Implement custom `Debug` for `AccountingErrorResponsePlan`. Keep only the
existing `status`, `code`, and `message` fields in formatter output.

## Consequences

Diagnostics keep the stable accounting error envelope. Future response-plan
fields must make redaction decisions explicitly instead of inheriting derived
`Debug`.
