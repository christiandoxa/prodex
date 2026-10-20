# Runtime Doctor Log-Tail Partial-Line Selection Migrated to Mojo (2026-10-20)

## Ownership

The production log_parser.mojo owner now selects the first LF boundary
when reading a persisted runtime log starting mid-file. The versioned
prodex_mojo_log_tail_boundary_v1 ABI scans original borrowed binary
bytes, preserving behavior for CRLF, no LF, malformed UTF-8, empty
tails and reads beginning at offset zero.

Rust retains path/file metadata, seek/read handling and application of
the checked offset to its owned Vec. Its previous byte-position search
and partial-line drain rule are deleted without a Rust policy fallback.

## Verification

- All 66 strict prodex-runtime-doctor tests pass, including direct
  temporary-file caller regressions for middle seeks, small/no
  limits, binary data and Unicode.
- Two direct core ABI tests pass for exact LF selection, invalid
  version/flags/pointers, and unchanged output on invalid calls.
- Direct Mojo ABI test, source anti-fallback guard/self-test, Cargo
  formatting, diff check and official share validation pass.

The checked production-share changes to 35.908872666164704%
(131,629 Mojo / 234,935 Rust production LOC). Non-regression is
satisfied; the 75% project target remains open. Metric definitions
and release floors have not changed. Remote CI green was not required.
