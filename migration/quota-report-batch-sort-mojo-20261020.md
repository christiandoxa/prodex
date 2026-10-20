# Quota Report Batch Sorting Migrated to Mojo (2026-10-20)

## Production ownership

The compiled quota.mojo owner now executes the complete stable sort of
quota report indices through prodex_quota_report_sort_batch_v1. The
production adapter marshals original report presentation facts into
borrowed scalar/string views and consumes the final permutation in one
versioned FFI call instead of asking a compiled pairwise comparator on
every Rust sort comparison.

Mojo owns heap-based O(n log n) index sorting across all six report
modes: current, remaining, profile, auth, account and plan. The original
compiled quota pairwise comparator remains the sole mode-specific
precedence authority. The final profile name tie-break uses exact
case-sensitive lexicographical UTF-8 bytes, matching the retired
Rust String::cmp behavior rather than the comparator's trimmed,
ASCII-case-folded column comparison. Exact complete ties preserve
original input order through an original-index tie-break.

Rust retains only quota provider I/O/data interpretation, the existing
compiled report-status projection, borrowed presentation-field
acquisition, and application of the compiled indices to the original
report references. The Rust indexes.sort_by() loop, comparator wrapper
and extra profile-name tie-break were deleted without a Rust semantic
sort fallback. The single-pair Mojo comparator remains separately
available as an existing public compiled API.

## Verification

- All 101 prodex-quota tests pass, including the real render caller
  regression with 2,049 profiles, six modes and identical profile
  names at widely separated input indices.
- Three direct prodex-mojo-core tests pass, comparing the new compiled
  sorting plan to the prior compiled pairwise comparator plus Rust-only
  test oracle for all modes, 2,049 observations and total-order
  adjacency; invalid ABI version, capacity and malformed late input
  rows are rejected before output writes.
- Direct Mojo ABI test passes for current, remaining and profile order,
  Unicode and capitalization, output integrity and bad inputs.
- No-fallback source guard/self-test, Cargo formatting, diff checks
  and official production-share non-regression checks pass.

Canonical broad share increases from 35.851405897242934% to
35.868414646488134% (131,136 Mojo / 234,467 Rust production lines).
The 75% project target remains unmet. Metric definitions, release
floor and non-regression baseline remain unchanged. No remote CI
green prerequisite.
