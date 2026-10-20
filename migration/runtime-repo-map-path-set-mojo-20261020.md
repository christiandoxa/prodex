# Repo-Map Unique Path Selection Migrated to Mojo (2026-10-20)

## Production ownership

The compiled runtime_repo_map.mojo owner now selects all nonblank,
lexically ordered, unique source paths from both file-location and
diff-hunk semantic ranges through the versioned
prodex_runtime_repo_map_path_set_v1 ABI.

The kernel applies the exact Unicode whitespace-only exclusion,
original-byte lexical ordering, and exact duplicate removal,
preserving original full path text when it contains meaningful
leading, trailing or internal whitespace. It reuses the existing
compiled repo-map lexical comparison and rich Unicode trim
operations, validates every row before any output mutation, and
returns stable original indices for the host to materialize.

Rust retains original Option<String> paths, source-range ordering
and String ownership, applies validated indices and constructs the
returned Vec<String>. Its former BTreeSet, path.trim().is_empty()
filter and path.clone() insertion policy have been deleted.
There is no Rust semantic fallback.

## Verification

- All 15 prodex-app artifact-store tests pass.
- Two new caller regressions verify file/diff union, Unicode
  whitespace, exact duplicates, blank/absent values and preservation
  of paths with meaningful surrounding/internal spaces.
- Two core ABI tests pass with direct sorted Unicode results,
  over 1,000 path views, malformed later tags and untouched results
  on rejected requests.
- Direct Mojo ABI test, source no-fallback guard/self-test,
  Cargo formatting and official production-share non-regression
  check pass.

Canonical share increases from 36.030173372161755% to
36.049461001902344% (133,029 Mojo / 235,989 Rust production LOC).
The separate 75% target remains unmet. Source count definitions,
historical release floors and remote CI prerequisites were not
weakened or changed.
