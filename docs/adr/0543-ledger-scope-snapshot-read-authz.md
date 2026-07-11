# ADR 0543: Authorize ledger reads from scope snapshots

## Status

Accepted

## Context

Scoped gateway admins filtered billing ledger records through the current
virtual-key store. A historical ledger row could disappear from an authorized
tenant's view after the key was deleted, or be evaluated against a later key
scope rather than the scope that admitted the request.

## Decision

When a billing ledger row contains tenant or governance scope snapshots, scoped
admin ledger reads authorize against that row snapshot. Older rows without a
snapshot continue to fall back to the current virtual-key store.

## Consequences

New file and Redis ledger rows can be read consistently by tenant-scoped admins
even if the key changes later. Legacy rows without scope snapshots remain hidden
from scoped admins when no matching key exists, preserving the safer historical
fallback.
