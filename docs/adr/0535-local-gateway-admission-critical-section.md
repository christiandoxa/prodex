# ADR 0535: Local Gateway Admission Critical Section

## Status

Accepted

## Context

Legacy gateway virtual-key admission used a cloned in-memory usage snapshot for
budget checks and then reacquired the usage lock to record accepted usage. That
split left a same-process read-modify-write race before the durable reservation
backend is fully wired into the gateway path.

## Decision

Keep the existing compatibility gateway path, but perform budget-group checks,
per-key admission checks, and local usage recording while holding the same usage
lock. If the lock cannot be acquired, fail closed with a budget denial instead
of admitting an unaccounted request.

## Consequences

This closes local lost-update admission races for the legacy in-memory gate. It
does not replace the enterprise durable reservation requirement; multi-replica
admission still must move behind the storage-backed atomic reservation boundary.
