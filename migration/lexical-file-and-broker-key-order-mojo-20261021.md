# Lexical Filesystem and Broker Key Ordering Migrated to Mojo (2026-10-21)

## Compiled ownership

The existing production core_file_policy.mojo owner now sorts original
borrowed byte views into ascending lexical order, with stable source
indices for exact duplicates and optional duplicate elimination. The
versioned prodex_core_file_lexical_order_v1 ABI validates all input
pointers/counts before modifying any output and intentionally accepts
binary filesystem name bytes where OS filenames are not UTF-8.

This one typed kernel is production reachable through prodex-core
for housekeeping, and directly through prodex-mojo-core for app
orchestration. It now owns the ordering previously duplicated in:

- Orphan managed profile directory selection in prodex-housekeeping.
- Runtime proxy log path discovery in prodex-housekeeping.
- Broker registry and direct live-log registry key enumeration.
- Broker cleanup artifact key selection.
- Live-log polling's combined broker/direct key deduplication.

Rust retains the directory read, symlink and file-type checks,
original PathBuf/String ownership, and all filesystem/network
side effects. All former sorting and deduplication calls in the
five production callers were removed; none survives as a Rust
semantic fallback. Selected source indices are validated before
materializing original host records.

## Verification

- 13 prodex-housekeeping tests pass, including two new filesystem
  regressions checking Unicode path ordering, symlink exclusions,
  and managed orphan directories.
- 13 prodex-core library tests pass.
- Two new app registry caller tests pass for Unicode, mixed broker/
  direct log keys, missing names and original lexical suffixes.
- Three live-log module tests pass, including real broker probing.
- Two direct core ABI tests cover Unicode/raw bytes, stable equal
  names, optional deduplication, 1,025 items, malformed late
  input rows and unchanged output on rejected requests.
- Direct Mojo ABI test, source no-fallback guard/self-test,
  Cargo formatting and canonical production-share non-regression
  checks pass.

Official production share rises from 36.09407450674974% to
36.12350969608325% (133,860 Mojo / 236,702 Rust production LOC).
The separate 75% project target remains unmet. Counting definitions,
the historical release floor and CI requirements are unchanged.
Remote CI green was not required to publish this checkpoint.
