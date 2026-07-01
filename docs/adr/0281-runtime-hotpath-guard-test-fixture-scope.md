# ADR 0281: Runtime Hotpath Guard Test Fixture Scope

## Status

Accepted.

## Context

The runtime hotpath guard protects the request and stream path from blocking
filesystem calls, unbounded thread spawns, and accidental blocking-pool use.
The default scan target includes `proxy_startup/` so newly split production
runtime modules remain covered.

That broad target also includes the local rewrite test harness
`proxy_startup/local_rewrite_tests.rs` and split fixtures under
`proxy_startup/local_rewrite_tests/`. Those tests intentionally create isolated
temporary directories, read audit logs, and start mock upstream servers after
requests complete. Treating fixture cleanup and assertions as production
hotpath violations makes the guard noisy and can hide real production matches.

## Decision

Exclude `proxy_startup/local_rewrite_tests.rs` and
`proxy_startup/local_rewrite_tests/` from the default runtime hotpath guard scan.
Explicit `--target` scans still honor the requested path, including test fixture
paths. Inline `#[cfg(test)]` stripping remains in place for test helpers
embedded in production files.

## Consequences

- The default CI guard reports production hotpath regressions instead of fixture
  disk I/O.
- Split test fixtures can keep asserting audit, ledger, and temporary-state
  behavior after proxy requests complete.
- Maintainers can still inspect fixture files deliberately by passing
  `--target`.
