# Runtime Smart Context Artifact Count/Byte Eviction Migrated to Mojo (2026-10-20)

## Production ownership

The compiled runtime_repo_map.mojo owner now chooses the entire
ordered eviction sequence for a persisted Smart Context artifact
store through versioned prodex_runtime_repo_artifact_eviction_v1.
It sorts by oldest artifact order first and lexical artifact ID
ascending on ties, then removes successive candidates until both
tracked total bytes and retained artifact count meet their limits.
Tracked bytes are subtracted saturating at zero, including historical
persisted stores where the recorded total differs from actual sum.

Rust still owns the original BTreeMap and artifact metadata, applies
the validated Mojo-selected source indices to original IDs,
removes those entries and updates the stored byte counter. The
previous Rust oldest-selection min_by comparator and count/byte
policy loop were deleted without a Rust semantic fallback.

The ABI accepts up to 1,048,576 initial records when normalizing
historical persisted input and validates every borrowed ID before
modifying scratch or output. It has no Mojo heap allocation.

## Verification

- All 15 existing artifact-store tests pass.
- Two new real-store caller tests cover 131 artifacts exceeding
  the 128-count limit, equal-order ID precedence, the 8-MiB
  byte cap and saturating subtraction.
- Three direct core tests verify count/byte constraints,
  Unicode ties, 1,025 records, invalid late ID input
  and untouched results on rejected calls.
- Direct Mojo test, no-fallback guard/self-test, Cargo formatting
  and official production-share non-regression pass.

Share rises from 35.970853048974604% to 35.98176843960252%
(132,310 Mojo / 235,404 Rust production LOC).
The 75% objective remains unmet. Counting definitions and
historic release floors were not altered. Remote CI green
was not a prerequisite.
