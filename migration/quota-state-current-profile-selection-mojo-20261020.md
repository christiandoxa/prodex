# Quota State Current-Profile Selection Migrated to Mojo (2026-10-20)

## Production ownership

The compiled quota.mojo owner now selects the current profile from
persisted last-run timestamps through the versioned
prodex_quota_state_current_profile_v1 ABI. It chooses the most
recent known profile, applies the lexically greatest UTF-8 profile
name on equal timestamps, disregards profile names missing from
the current state, and selects the configured active profile only
if no eligible last-run observation exists and the active profile
is known.

Rust retains the original AppState/profile maps, borrows original
profile names and signed timestamps, acquires name-availability
facts and materializes the Mojo-selected original profile.
The previous Rust filter/max_by comparator and local active
fallback branch were deleted. No Rust semantic duplicate/fallback
remains.

## Verification

- All 16 strict prodex-app quota_support tests pass, including two
  new real caller regressions for Unicode ties, negative and
  extreme timestamps, deleted profiles, missing active profiles,
  and an empty profile registry.
- Two direct core ABI tests verify result tags, signed time
  precedence, missing candidate/active fallback, malformed
  late availability tags, pointer/version limits and untouched
  output on invalid requests.
- Direct compiled Mojo regression, source no-fallback guard/self-test,
  Cargo formatting and canonical production-share non-regression pass.

Official broad share rises from 35.996325158260106% to
36.00180409780166% (132,505 Mojo / 235,546 Rust production LOC).
The 75% project target remains unmet. No accounting definitions,
historical release floors or non-regression checks were changed.
Remote CI green was not a prerequisite.
