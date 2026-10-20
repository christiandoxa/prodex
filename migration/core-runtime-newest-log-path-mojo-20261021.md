# Core Newest Runtime Log Selection Migrated to Mojo (2026-10-21)

## Production ownership

The existing compiled log_semantics.mojo recent-path sort kernel now
owns the third production mode: unsigned u128 modification timestamps
descending, raw path bytes descending on timestamp ties, and later
original index first for exact timestamp/path ties. This replaces
prodex-core's Rust max_by(timestamp, PathBuf) comparator used by the
housekeeping runtime-log discovery path.

The existing typed log path planner is reused rather than duplicating
the sorting algorithm. prodex-core now enables its explicit
mojo-routing and mojo-rich dependencies, acquires original PathBuf
bytes, and applies the compiled selected source index.

For an input exceeding the kernel's 65,536-row limit, the host asks
the same compiled owner to select one candidate per bounded batch,
then runs a second compiled selection over the finalists. The host
does not implement a comparison, sort, or fallback. This preserves
the last occurrence of an exactly equal timestamp/path candidate
across batch boundaries.

## Verification

- All 13 prodex-core unit tests pass, including two new regressions
  covering u128::MAX, Unicode lexical ties and 65,537 candidates.
- All 11 prodex-housekeeping tests pass, including two new real
  filesystem caller regressions covering timestamp precedence, path
  ties, and empty directories.
- Four direct compiled-core recent-path tests pass, including the
  existing session and runtime-info modes.
- Previous existing session-log and runtime-info caller regressions
  pass (two and one respectively). The direct Mojo ABI test,
  anti-fallback guard/self-test, Cargo formatting and canonical
  production-share check pass.

The measured production share is 36.09241084705437%
(133,495 Mojo / 236,375 Rust production LOC) compared with
36.09557853295551% in the preceding checkpoint. This small reduction
comes from explicit host-owned input batching and index materialization,
not any Rust semantic fallback. The audited release-floor
non-regression gate passes. The 75% project target remains unmet.
The source-count definitions and release floors are unchanged.
Remote CI green was not a checkpoint prerequisite.
