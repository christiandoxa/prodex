# ADR 0005: Ledger identity uses call ID

## Status

Accepted.

## Context

Phase 0 audit called out billing ledger uniqueness based on process-local
request IDs. Multi-replica deployments can collide if two gateway instances
generate the same numeric request sequence or replay old state.

Prodex is incrementally moving to typed globally unique identifiers. Runtime
logs still carry numeric request IDs for compatibility, while `call_id` is the
billing/idempotency-facing identifier exposed by gateway accounting.

## Decision

New SQL billing ledger schemas use `UNIQUE(call_id, key_name, phase)` instead of
`UNIQUE(request_id, key_name, phase)`. Startup also creates a matching unique
index for compatibility with existing tables. SQL ledger insertion uses `call_id` as the idempotency conflict target, and
SQL response reconciliation now matches ledger entries by `call_id`, not by
request ID.

## Consequences

- Billing reconciliation no longer updates an unrelated row solely because a
  numeric request ID collided.
- Numeric request IDs remain available for log correlation and compatibility.
- Existing SQLite tables that already contain the old inline unique constraint
  still need a future expand/contract migration to fully remove that legacy
  constraint; the added call-ID index and reconciliation behavior are the Phase 0
  compatibility-preserving hardening step.
