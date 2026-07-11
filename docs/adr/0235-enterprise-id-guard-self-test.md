# 0235: Enterprise ID Guard Self-Test

## Status

Accepted

## Context

Enterprise request, call, reservation, tenant, principal, virtual-key, policy,
and audit identifiers must remain typed and globally unique. The CI guard for
this boundary already scans enterprise crates for process-local `AtomicU64` and
monotonic `fetch_add` identity generation, but the guard itself also needs
regression coverage.

## Decision

`scripts/ci/enterprise-id-boundary-guard.mjs` now runs self-tests before scanning
the workspace. The self-tests prove that the guard:

- accepts the complete UUIDv7 typed ID set;
- rejects a non-UUIDv7 ID generator;
- rejects a missing typed ID;
- rejects `AtomicU64` and `.fetch_add(...)` in enterprise boundary source;
- accepts typed `RequestId::new()` usage.

## Consequences

- CI fails if the ID boundary guard stops detecting process-local identity
  regressions.
- Runtime compatibility counters remain outside this enterprise boundary guard
  until those wire/log contracts are migrated deliberately.
