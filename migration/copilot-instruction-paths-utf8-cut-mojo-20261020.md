# Copilot Custom Instructions Path Ordering and UTF-8 Clipping Migrated to Mojo (2026-10-20)

## Production ownership

The production launch_config.mojo owner now determines lexical filesystem
path order for Copilot custom-instruction discovery and the post-lossy UTF-8
byte limit for each instruction file. Two versioned compiled ABIs are used
by the live launch workflow:

- prodex_launch_instruction_path_order_v1 lexically sorts original
  borrowed path-encoded bytes, preserving original order for identical
  paths. It is used for each directory's discovery order and again for
  the global/scoped combined order before the 32-file cap is applied.
- prodex_launch_instruction_text_cut_v1 selects the last complete UTF-8
  scalar boundary at or before the allowed post-decode byte limit, exactly
  retaining the previous behavior of lossy decode followed by clipping.

Rust retains directory discovery, symlink checks (including parent
components), file-type and extension checks, bounded file reads, lossy
UTF-8 transport acquisition and final instruction text materialization.
Both former Rust sorts and the local is_char_boundary decrement loop
were deleted rather than kept as fallbacks. The Mojo adapter verifies
that the returned cut is an original valid string boundary.

## Verification

- All 57 prodex-app Copilot tests pass, including two new real caller
  regressions for global-file priority, the first 31 of 40 scoped
  files under the shared cap, lexical filename order, incomplete
  multibyte emoji at a 64-KiB read cutoff, and complete emoji ending
  exactly at the limit.
- Three direct compiled-core tests pass for Unicode/equal-path order,
  1,025 entries, scalar boundaries, malformed late fields, invalid
  versions/pointers and untouched outputs on invalid ABI calls.
- Direct Mojo ABI test, source no-fallback guard/self-test, Cargo
  formatting and official production-share non-regression checks pass.

Official broad share rises from 36.00180409780166% to
36.01139399995655% (132,617 Mojo / 235,647 Rust production LOC).
The separate 75% goal remains unmet. Source counts and historical
release-floor rules were unchanged. Remote CI green was not a
checkpoint prerequisite.
