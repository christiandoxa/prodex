# Runtime Lineage Latest Turn-State Binding Selection Migrated to Mojo (2026-10-21)

## Production ownership

The existing compiled runtime_lineage.mojo owner now selects the
most recent turn-state binding for a requested previous response,
subject to an optional exact bound-profile filter, through
prodex_runtime_lineage_latest_turn_binding_v1.

The caller first acquires response and turn-state components using
the existing compiled lineage key parser. The new batch then performs
exact response/profile matching, compares signed bound_at timestamps,
and selects the latest original input index on timestamp ties. It
validates the complete set of borrowed UTF-8 row views before any
output mutation and returns -1 when no candidate qualifies.

Rust retains original lineage binding metadata, parser-output
references and final original turn-state String materialization.
The previous Rust response-id prefix check, profile filter,
candidate tuple construction and greatest-timestamp/last-tie
selection were deleted from the live path without a Rust semantic
fallback. A separate test-only drain fixture retains its own
prefix construction solely for its test contract, not as a
production selection fallback.

When input contains more than 65,536 bindings, Rust applies the
same compiled selector once per bounded batch and once to the
selected finalists; it does not reimplement the comparator.

## Verification

- All 427 prodex-runtime-proxy tests pass, including two new
  caller regressions covering exact Unicode identity/profile,
  malformed lineage keys, signed times, equal-time last original
  tie and 65,537 input bindings.
- Two prodex-app runtime-proxy lineage integration tests pass.
- Two core direct ABI tests cover parsed facts, missing filters,
  Unicode, malformed late rows, extreme timestamps and unchanged
  output on rejected calls.
- Direct Mojo ABI test, strict anti-fallback guard/self-test,
  Cargo formatting and canonical production-share non-regression
  checks pass.

Official share rises from 36.09241084705437% to
36.102403908513125% (133,602 Mojo / 236,462 Rust production LOC).
The 75% objective remains unmet. No counting/release-floor
rules were weakened. Green remote CI was not a prerequisite.
