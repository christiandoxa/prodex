# Smart Context Duplicate Chunk Grouping Migrated to Mojo (2026-10-21)

## Production ownership

The existing compiled runtime_repo_map.mojo owner now groups all
semantic chunk fingerprint observations with a single versioned
prodex_runtime_repo_duplicate_groups_v1 ABI. It lexically sorts
content hashes, then sorts chunk byte lengths within each hash, and
uses original chunk traversal indices as a stable tie break.
The compiled kernel selects only groups with two or more
occurrences, returns the first eight original occurrences per group
and at most 64 duplicate fingerprint groups, and marks the result
incomplete when another eligible group or occurrence is truncated.

Rust retains original chunk String/semantic range records and
materializes selected occurrence (start, end, kind) metadata from
validated original indices. The previous Rust BTreeMap grouping,
per-group duplicate selection loop, superseded scalar duplicate
ABI and its Rust wrapper were deleted; no Rust semantic fallback
remains. The new kernel validates all borrowed UTF-8 views and row
bounds before mutating outputs.

## Verification

- All 15 runtime-state artifact-store tests pass.
- Two new real caller regressions verify hash/length key order,
  source-traversal order, original occurrence kinds, 64-group
  truncation, eight-occurrence cap and empty input.
- Three core batch tests cover group boundaries, multiple sizes
  under one hash, complete/incomplete flags, malformed late
  UTF-8 views and unchanged output on rejected ABI requests.
- Direct Mojo ABI tests, strict source anti-fallback guard,
  Cargo formatting and official production-share non-regression
  checks pass.

Official broad production share rises from 36.07194088829946% to
36.091252613612696% (133,429 Mojo / 236,270 Rust production LOC).
The 75% objective remains unmet. Counting definitions and historical
release floors are unchanged. Remote CI green was not required.
