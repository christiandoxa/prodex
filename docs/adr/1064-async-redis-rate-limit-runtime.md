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
conversion, and stable redacted errors. It executes both atomic single-counter
and all-or-nothing RPM/TPM plans and maps results through the planning crate's
typed result parsers. Dual-counter keys use an internal tenant hash tag so both
keys share one Redis Cluster slot.

Wire the dual plan into active PostgreSQL-backed gateway admission before
durable reservation and upstream dispatch. Two independently connected
executors prove the Redis primitive does not overshoot or consume RPM when TPM
denies a request. ADR 1066 adds durable grouped request-budget admission, so the
multi-replica production gate now accepts a valid PostgreSQL+Redis topology with
at least two replicas and accounting checks enabled.

## Consequences

Redis remains rebuildable coordination rather than durable billing state.
PostgreSQL remains the source of truth for reservations, counters, and ledger
events. PostgreSQL TLS remains a separate production-readiness gap.
