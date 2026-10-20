# Runtime Doctor Partial Log-Tail Boundary Migrated to Mojo (2026-10-20)

## Production semantics

The existing log_parser.mojo owner now finds the first LF byte in
a log tail and determines the precise number of bytes to discard
when a file read began in the middle. If the tail started at byte
zero or if there is no LF in a truncated tail, the kernel preserves
all bytes. Only the versioned prodex_mojo_log_tail_boundary_v1
operation chooses this byte range, preserving existing behavior for
binary prefixes, LF and CRLF, UTF-8, and empty files.

Rust still opens and seeks files, acquires and retains the original
byte buffers, validates the compiled return value, drains the selected
prefix and handles I/O errors. The old Rust iter/position scan and
partial-line decision were removed, with no semantic fallback.

## Verification

- All 66 prodex-runtime-doctor tests pass, including the real
  filesystem caller test for a complete file, partial first line,
  no-newline tail, empty read, Unicode and binary prefix.
- Two direct core ABI tests pass, including negative inputs, bad
  version/flags and no output mutation after rejection.
- Direct Mojo ABI test, anti-fallback source guard/self-test,
  Cargo formatting and official production-share non-regression
  checks all pass.

Official production share rises from 35.90709045120557% to
35.908872666164704% (131,629 Mojo / 234,935 Rust production LOC).
The project target of 75% is still unmet. No counting rule,
historic release floor or CI policy was changed, and remote CI
green was not required to publish this checkpoint.
