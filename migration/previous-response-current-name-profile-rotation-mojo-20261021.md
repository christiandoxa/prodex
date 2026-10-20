# Previous-Response Current-Name Profile Rotation Migrated to Mojo (2026-10-21)

## Production ownership

The compiled profile_schedule.mojo owner now performs exact current-profile
name matching and provider-priority rotation together through the
versioned prodex_runtime_profile_rotation_by_current_name_v1 ABI. The
kernel validates borrowed UTF-8 names and nonnegative priorities,
selects the first exact current name, and reuses its existing compiled
rotation ordering. Missing current names produce the existing
provider-priority order; equal-priority ties preserve current-relative
stable order.

Rust retains original AppState BTreeMap profile records and provider
priority acquisition, then applies Mojo-selected original indices.
The previous Rust .position(|(_, name, _)| name == current_profile)
lookup and caller-side current-index branch were deleted. The older
compiled-Mojo rotation adapter is kept only for a distinct quota
caller, not as a Rust semantic fallback in the previous-response path.

## Verification

- All 79 prodex-app previous-response tests pass.
- Two core ABI tests exercise Unicode names, duplicates, missing and
  empty current names, priority ties, invalid row views and unchanged
  output on rejected ABI requests.
- Direct Mojo ABI test, anti-fallback guard/self-test, Cargo formatting
  and canonical production-share non-regression checks pass.

The official broad production share increases from 36.06472593559358%
to 36.07194088829946% (133,275 Mojo / 236,195 Rust production LOC).
The 75% project target remains unmet. Counting definitions and release
floors are unchanged, and remote CI green is not a prerequisite.
