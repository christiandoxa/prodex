# Recent Session and Runtime-Info Log Path Ordering Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled log_semantics.mojo owner now orders and selects
the latest file candidates through a versioned
prodex_mojo_recent_log_paths_v1 batch ABI. Both original callers use
the same heap-sort owner with different, preserved legacy policies:

- Session log discovery: modification timestamp rounded to Unix epoch
  milliseconds (including 128-bit millis), newest first and path
  ascending on equal timestamps, with stable original order on exact
  ties, keeping at most 32 session logs.
- Runtime-info log selection: full signed SystemTime with subsecond
  nanosecond precision, newest first, path descending on ties, and
  reverse original input order for an exact time/path tie, preserving
  the previous ascending-sort-and-reverse behavior.

Rust retains filesystem discovery and modified-time metadata
acquisition and applies the validated Mojo-selected source indices
to original PathBuf values. Both previous Rust sorting/truncation
policies were deleted without fallback. The compiled kernel validates
the full borrowed input before modifying output and uses bounded
host-owned buffers.

## Validation

- All 129 prodex-app log module tests pass.
- All 23 prodex-app reports::info tests pass, including real callers
  that exercise Unicode tie paths, signed pre-epoch times, and
  nanosecond ordering.
- Three direct core ABI tests verify both modes, empty/overlong
  requested limits, large millisecond epochs, invalid row shapes,
  output bounds and unchanged output on rejection.
- Direct Mojo ABI regression, source no-fallback guard/self-test,
  Cargo formatting and audited production-share non-regression
  checks pass.

Official production share increases from 35.95155464268008% to
35.96076947575585% (132,108 Mojo / 235,259 Rust production LOC).
The 75% project target is still unmet. No source-count accounting,
release floor or historical non-regression baseline was changed.
Remote CI green was not required to publish this checkpoint.
