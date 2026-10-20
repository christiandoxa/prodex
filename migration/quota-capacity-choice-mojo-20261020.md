# Quota Capacity Candidate Selection Migrated to Mojo (2026-10-20)

## Production ownership

The compiled quota owner now selects the first routing-eligible quota
capacity candidate and, when no routing-eligible candidate exists,
falls back to the first main quota lane. The single versioned
prodex_quota_capacity_choice_v1 ABI also emits whether any candidate
is eligible. It validates every lane/eligibility tag before committing
its result. The input may contain more than a single 256-item
capacity-classification batch.

Rust still acquires original provider capacity observations, executes
the existing compiled capacity classifier, and maps a Mojo-selected
index to the original borrowed WindowPair. The previous Rust
find/or_else/any selection predicates in the two live quota window
callers were deleted. Invalid Mojo results fail explicitly, without
a Rust fallback. No provider-account identifiers or secrets are
introduced into this ABI.

## Validation and metrics

- All 100 prodex-quota tests pass, including new real caller tests
  for absent windows, unknown additional lanes, ready main lanes,
  and main-lane fallback while blocked.
- Two direct core ABI tests pass: first-ready and main fallback
  ordering, absent candidates, 1025 candidates, malformed tags,
  incorrect version and unchanged outputs on invalid calls.
- Direct Mojo test, source no-fallback guard, formatting, diff check
  and official production-share non-regression check pass.

The official broad share changed from 35.82742660744385% to
35.826879696153064% (130,551 Mojo / 233,843 Rust production LOC).
The small percentage decrease is due to the checked typed Rust
adapter; the semantic decision is now compiled Mojo-owned.
Project target 75% is still unmet. No metric definition, release
floor, or non-regression baseline was weakened. Remote CI was not
a checkpoint prerequisite.
