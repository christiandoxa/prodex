# 1064: Async Redis Rate-Limit Runtime

## Status

Accepted.

## Context

`prodex-storage-redis` owns driver-free, tenant-scoped Lua plans but the active
gateway needs a bounded async executor before distributed admission can replace
process-local RPM and TPM checks.

## Decision

Add `prodex-storage-redis-runtime` as the Redis driver boundary. It owns one
reconnecting async connection manager, bounded connection and response
timeouts, `redis://` and rustls-backed `rediss://` support, checked integer
conversion, and stable redacted errors. It executes the existing atomic
single-counter plan and maps results through the planning crate's typed result
parser.

Keep the multi-replica production gate closed. Separate RPM, TPM, and grouped
budget scripts could consume partial allowance, so active enterprise admission
must wait for one atomic multi-counter Lua plan and two independently connected
gateway instances proving no overshoot.

## Consequences

Redis remains rebuildable coordination rather than durable billing state.
PostgreSQL remains the source of truth for reservations, counters, and ledger
events. The runtime crate can be tested independently before gateway wiring.
