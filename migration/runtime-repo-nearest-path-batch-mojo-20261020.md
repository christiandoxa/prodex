# Smart Context Repo-Map Nearest-Path Selection Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled runtime_repo_map.mojo owner now chooses the
nearest source path for a repo-map symbol or error candidate through
the versioned prodex_runtime_repo_nearest_path_v1 batch ABI. It
applies the existing compiled path-distance function across all
original file-location and diff-hunk observations in one compiled call,
choosing minimum distance first, earliest range start on a distance
tie, and first original row when those are also equal.

The host keeps the original semantic index and optional String
paths, borrows each range's start/end and presence facts, then
materializes the selected original path. Missing-path candidates
are skipped without accidental range validation, preserving the old
filter_map behavior; present empty path strings remain valid.
The previous Rust filter_map, repeated per-range compiled distance
calls, cloning of each candidate's path, and Rust min_by selection
policy were deleted. No fallback reproduces the decision in Rust.

## Validation

- All 15 strict runtime-state artifact-store tests pass, including
  live symbol/path projection callers using the compiled policy.
- Two new production caller regressions test file and diff ranges,
  equal-distance/lowest-start selection, first encountered exact
  ties, Unicode paths, absent paths, and present empty paths.
- Two direct compiled-core tests pass, covering 1,025 rows,
  empty inputs, malformed late flags, invalid intervals and
  untouched outputs on rejection.
- Direct Mojo ABI tests, updated source anti-fallback guard/self-test,
  Cargo formatting and canonical production-share non-regression
  checks pass.

Official broad production share increases from 36.01139399995655%
to 36.01500636576116% (132,671 Mojo / 235,706 Rust production LOC).
The 75% project goal remains unmet. Count definitions and historical
release floors were not weakened; remote CI green was not required.
