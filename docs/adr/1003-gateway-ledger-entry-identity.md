# ADR 1003: Gateway ledger entry identity

## Status

Accepted.

## Context

Phase 0 accounting requirements require ledger uniqueness to be stable across
replicas. SQL backends use the exact tuple `(call_id, key_name, phase)` for
billing ledger idempotency, while the file and Redis backends built their
de-duplication keys by lowercasing `key_name` and joining fields with `:`.
That could collapse distinct key names such as `Alpha` and `alpha`, or create
delimiter ambiguity if a future identifier contains `:`.

## Decision

File and Redis billing ledger backends now share an exact, length-prefixed
ledger entry identity derived from `call_id`, `key_name`, and `phase`. The
identity preserves case and field boundaries, matching the SQL uniqueness
semantics without introducing a new dependency.

## Consequences

Duplicate request-phase ledger events for the same exact call/key/phase remain
idempotent. Distinct key names and delimiter-bearing fields no longer collide in
file or Redis de-duplication. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_ledger_types.rs`,
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_file_ledger.rs`,
and
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_redis_ledger.rs`.
