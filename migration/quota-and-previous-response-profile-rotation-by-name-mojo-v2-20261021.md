# Unified Quota / Previous-Response Current-Name Rotation in Mojo (2026-10-21)

## Production ownership

The compiled profile_schedule.mojo owner now performs first-exact
current-profile name matching and provider-priority rotation for
both the quota selection and previous-response routing callers
through one versioned ABI:
prodex_runtime_profile_rotation_by_current_name_v2.

The ABI models three distinct production contracts:
- Exclude: do not emit an existing current profile; when absent,
  return the ordinary provider-priority order.
- Include: include an existing current profile; when absent,
  add one synthetic current-profile index with the established
  last provider priority. It is rejected when it would exceed the
  compiled scheduler's 256-profile maximum.
- IncludeIfPresent: the previous-response route includes the
  current profile only when it is actually one of the known rows.
  A missing current name does not synthesize a phantom route.

Compiled Mojo validates every borrowed UTF-8 view and nonnegative
provider priority before any output writes. It locates the first
identical name, derives the effective inclusion contract, checks
the required output capacity, and reuses the existing compiled
rotation order kernel.

Rust retains original AppState/profile records and provider-priority
acquisition. Both live callers apply Mojo-selected original indices;
quota additionally materializes the special synthetic-current index.
The Rust current-name .position comparison in the quota caller is
deleted. The earlier by-name v1 ABI is retired rather than preserved
as a parallel semantic implementation, and no Rust semantic fallback
exists in either caller.

## Verification

- All 30 prodex-runtime-quota tests pass, including two new quota
  caller regressions testing synthetic missing current, exclusion,
  Unicode exact name matching, first duplicate name and empty sets.
- All 79 prodex-app previous-response tests pass.
- Three compiled-core ABI tests pass for every inclusion mode,
  empty/duplicate names, malformed late UTF-8/name fields, invalid
  mode/capacity, synthetic 257th profile rejection and nonmutating
  rejected calls.
- Direct Mojo ABI regression, source no-fallback guard/self-test,
  Cargo formatting, and canonical production-share non-regression
  checks pass.

The official production-share increases from 36.09407450674974%
to 36.09557853295551% (133,493 Mojo / 236,339 Rust LOC).
The 75% project target remains unmet. The 7% release floor,
historical baselines and production source counting are unchanged.
Remote CI green was not required before pushing this checkpoint.
