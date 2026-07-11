# 0197: Domain Audit Retention Decision

## Status

Accepted

## Context

Retention cleanup should not delete an audit event merely because it is older
than the retention cutoff. Legal holds can override purge eligibility, and the
decision must be tenant-scoped and timestamp-validated before storage adapters
execute deletes.

If purge decisions are assembled in each backend, hold checks and redacted
error handling can drift across PostgreSQL, SQLite, and any future storage
implementation.

## Decision

`prodex-domain` owns `AuditRetentionDecision` and
`plan_audit_retention_decision_error_response`.

`AuditRetentionPlan::purge_decision` first evaluates expiry with the existing
retention plan. Recent events are retained. Expired events protected by an
active `AuditRetentionHold` are marked `protected_by_hold`; only expired,
unheld events are marked `purge`. Retention and hold errors delegate to the
existing redacted response planners.

## Consequences

- Storage adapters can consume a single purge/retain/protected decision.
- Legal holds are enforced before delete execution.
- Tenant IDs, event IDs, timestamps, hold internals, and storage details stay
  out of client-visible retention decision errors.
