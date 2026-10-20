# Child Process Output Line Framing Migrated to Mojo (2026-10-20)

## Production ownership

The compiled launch_config.mojo owner now selects all LF byte offsets
after the original scan cursor in a child process stdout/stderr output
buffer through versioned prodex_launch_process_line_ends_v1. It does
not require UTF-8 and preserves binary bytes, empty lines, incomplete
trailing lines and incremental fragments. The two-pass ABI measures
and then emits exact absolute newline indices, rejecting insufficient
capacity without writing partial output.

Rust retains bounded Read and pipe ownership, the exact original
line_match callback invocation and first-match timestamp behavior,
end-of-stream partial-line matching, child termination and error
handling. The old Rust bytes[line_start..].iter().position LF search
and repeated per-line relative offset math were deleted without
a Rust semantic fallback.

## Verification

- All 23 prodex-app ping tests pass, including subprocess launch,
  result latency and bounded-output caller cases.
- Two new application regressions pass, exercising one-byte reads,
  binary UTF-8-invalid output, multiple completed lines in one
  read, first-match-only timing, complete and incomplete final
  lines, bounded reads, and empty/nonmatching output.
- Two core ABI tests cover binary/empty inputs, 1,025 line ends,
  malformed version/pointer/offset/count flags and unchanged
  outputs on rejected requests.
- Direct Mojo ABI test, source anti-fallback guard/self-test,
  Cargo formatting and canonical Mojo production-share
  non-regression checks pass.

Official share increases from 36.02996620393727% to
36.030173372161755% (132,880 Mojo / 235,922 Rust production LOC).
The 75% goal remains unmet. Counting definitions and historical
release floors were not changed. Remote CI green was not a
prerequisite to checkpoint publication.
