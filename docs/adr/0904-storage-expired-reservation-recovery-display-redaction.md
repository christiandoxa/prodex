# ADR 0904: Storage expired reservation recovery display redaction

## Status

Accepted.

## Context

Expired reservation recovery rejects cross-tenant commands, records that are
not expired, and invalid reservation records before storage adapters release
reserved capacity. The response planner already exposes one stable rejection
message, but raw `Display` output still distinguished not-expired records from
invalid records.

## Decision

Render `ExpiredReservationRecoveryPlanError` with the same message used by
`plan_expired_reservation_recovery_error_response`. Keep typed variants
unchanged for trusted classification.

Regression coverage pins exact display strings for tenant mismatch,
not-expired, and invalid-record cases.

## Consequences

Storage recovery workers and composition roots can safely fall back to raw
display strings without exposing reservation state or record validity details.
Diagnostics should continue matching typed variants for exact reasons.
