# Runtime Smart Context Repo-Map Artifact Priority Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled runtime_repo_map.mojo owner now chooses the
priority ordering for artifacts when generating the live repository and
symbol map projections. The versioned
prodex_runtime_repo_artifact_order_v1 ABI sorts borrowed records with
the newest artifact order first, lexical artifact ID ascending on
equal order, and stable input index on exact equality. It reuses the
existing compiled repo-map bytewise lexical comparator and uses a
bounded, host-owned output index buffer with iterative heapsort.

Rust still acquires the original BTreeMap artifacts, owns all path,
symbol and error-range materialization and applies only the validated
Mojo-selected indices. The former Rust artifacts.sort_by comparison
and source-order selection are deleted without a Rust semantic
fallback.

## Verification

- All 15 runtime-state shared artifact-store tests pass.
- Two new production caller regressions pass, exercising conflicting
  repo-map path artifacts, reversed latest order, equal-order
  artifact-ID tie-breaking, and symbol-map projection.
- Two direct compiled-core tests pass with Unicode IDs, duplicate
  equal-order IDs, 1,025 records, malformed late row lengths, output
  immutability on rejection and empty results.
- Direct Mojo ABI test, anti-fallback guard/self-test, Cargo
  formatting and canonical production-share non-regression pass.

The official share increases from 35.96076947575585% to
35.970853048974604% (132,199 Mojo / 235,318 Rust production LOC).
The separate 75% goal remains unmet. Counting definitions, the
historic release floor and no-fallback checks were not weakened.
Remote CI green was not a prerequisite.
