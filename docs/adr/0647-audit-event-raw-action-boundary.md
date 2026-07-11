# 0647: Audit Event Raw Action Boundary

## Status

Accepted

## Context

`AuditAction::try_new` validates stable machine-readable audit action names,
but the legacy `AuditAction::new` constructor remains unchecked for
compatibility. `AuditEvent::new` persisted that unchecked action directly.

## Decision

`AuditEvent::new` now stores the provided action only when it passes the
`AuditAction` validator. Invalid raw actions are replaced with the stable
low-cardinality action `audit.invalid_action`.

## Consequences

- Audit records do not persist request-controlled action names.
- Existing callers keep the same constructor signature.
- Invalid action values remain visible as a stable sentinel without leaking raw
  input.
