# ADR 0954: Storage Accounting Error Debug Redaction

## Status

Accepted

## Context

Atomic reservation and usage reconciliation planner errors can carry tenant
identifiers and usage amounts. Display output is generic, but derived debug
output would expose those values in diagnostics.

## Decision

Use custom `Debug` implementations for `AtomicReservationPlanError` and
`UsageReconciliationPlanError`. Redact tenant identifiers and usage amounts
while preserving error variant names.

## Consequences

Storage diagnostics can distinguish accounting planner failures without leaking
tenant identifiers or usage values from mismatch and arithmetic errors.
