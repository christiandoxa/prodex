# Smart Context Scope-Store Retention Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled smart_context_policy.mojo owner now selects the
ordered scope-store removals, using a versioned batch ABI:
prodex_smart_context_store_retention_v1. The algorithm implements
chronological modification-time sorting (including dates before the
Unix epoch), stable raw-path-byte tie-breaking, strict elapsed
retention comparison at nanosecond resolution, cumulative disk byte
budgeting, and oldest-first removal. Inputs are bounded and validated
before any output changes; the compiled policy does not allocate a
Mojo heap and sorts host-provided original indices in place.

Rust retains filesystem directory enumeration, regular-file checks,
SystemTime metadata acquisition, borrowed path bytes, and the
execution of each selected filesystem removal. The previous
Rust sort_by chronological/path comparator, expired threshold
branch, and running disk-cap deletion-policy loop were deleted
without an alternate Rust decision or fallback. Filesystem errors
still abort the caller, preserving the original side-effect contract.

## Verification

- All 117 prodex-app Smart Context tests pass, including three new
  actual filesystem caller regressions for chronological/disk
  evictions, Unicode path ordering, exact expiry threshold,
  future timestamps, non-existing roots and repeated pruning.
- Three direct core ABI tests pass for full ordering, path tie-break,
  nanosecond strictness, pre-epoch timestamps, malformed late
  input fields and unchanged outputs on rejected calls.
- Direct Mojo ABI tests, source anti-fallback guard/self-test, Cargo
  formatting, diff checks and canonical Mojo share non-regression
  checks pass.

Official production share rises from 35.908872666164704% to
35.921607541552966% (131,768 Mojo / 235,053 Rust production LOC).
The 75% project objective remains unmet. Metric definitions and
the historical release floor were not modified. Remote CI green
was not a prerequisite. A separate attempt to run the entire app
test suite could not be completed by the execution tool; only the
117 relevant app tests are claimed here.
