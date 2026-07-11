# ADR 0765: Redact reservation commit error debug output

## Status

Accepted

## Context

`ReservationCommitError` carries commit mismatches, tenant-owned usage amounts,
and overflow details for trusted accounting decisions. Derived `Debug` exposed
those values in diagnostics.

## Decision

Implement custom `Debug` for `ReservationCommitError`. Preserve the error
variant shape and delegate mismatch redaction to `ReservationCommitMismatch`,
while redacting all usage amount fields.

## Consequences

Diagnostics still show which commit invariant failed. Tenant identifiers and
usage amounts no longer appear through commit-error debug formatting.
