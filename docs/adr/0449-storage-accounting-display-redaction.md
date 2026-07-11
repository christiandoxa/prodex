# ADR 0449: Storage accounting display errors are redacted

## Status

Accepted.

## Context

Storage response planners already return stable redacted envelopes for atomic
reservation and usage reconciliation failures. Their local `Display` text still
included tenant IDs and usage amounts, which could leak through accidental
display paths.

## Decision

`AtomicReservationPlanError` and `UsageReconciliationPlanError` now use generic
display messages for tenant mismatch and usage arithmetic failures. Structured
enum fields remain available for trusted diagnostics and tests.

## Consequences

Storage/accounting validation failures no longer expose tenant identifiers,
reserved usage, actual usage, or committed usage through generic display text.
