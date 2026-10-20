# Runtime Followed-Log Byte Framing Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled live_log_record.mojo owner now determines
two remaining file-follow framing decisions using versioned ABIs:

- prodex_live_log_follow_last_newline_v1 scans binary pending bytes
  and returns the exact offset after the last complete LF-terminated
  line, preserving incomplete UTF-8 scalars in the trailing buffer.
- prodex_live_log_follow_line_lengths_v1 scans the host's historical
  lossy UTF-8 decoded complete prefix, returning inclusive line
  lengths for the already compiled follow_line_plan; a separate
  measurement call verifies the output capacity before any write.

Rust retains file I/O, original pending bytes, lossy UTF-8 decoding,
application of Mojo-selected line keep/drop flags, gap marker
materialization and file rotation behavior. Its old Rust
last-LF rposition scan and split_inclusive length acquisition were
removed without semantic fallback. The existing live-log length and
overflow policies remain compiled in Mojo.

## Verification

- All 8 prodex-app log-follow module tests pass, including two
  new production caller regressions for incomplete multibyte
  boundaries, invalid binary UTF-8 replacements, CRLF, repeated
  blank lines, and preservation of trailing partial lines.
- Three compiled-core ABI tests pass, covering binary last-LF
  offsets, exact UTF-8 split-inclusive lengths, 1,025 lines,
  rejected malformed UTF-8, bad ABI versions/capacity and untouched
  results on rejected calls.
- Direct Mojo ABI regression, source no-fallback guard/self-test,
  Cargo formatting and official production-share non-regression
  validation pass.

Official broad share increases from 36.02408461634476% to
36.02996620393727% (132,836 Mojo / 235,846 Rust production LOC).
The 75% project goal remains unmet. Accounting definitions and
historical release floors remain unchanged; green remote CI
was not required to publish the checkpoint.
