# Live-log in-memory eviction semantics migrated to Mojo (2026-10-20)

## Production ownership and hot-path design

The existing compiled live-log record owner now selects both per-path and
global eviction conditions through the versioned scalar ABI
prodex_live_log_retention_trim_v1. Limits for path entries/bytes and total
entries/bytes/paths are no longer duplicated as Rust eviction predicates.
The versioned prodex_live_log_oldest_path_v1 selects the path with the least
last_sequence; equal sequences retain the first caller-supplied key order,
matching the existing BTreeMap behavior. Rust retains the Mutex, VecDeque,
BTreeMap, update counters, and actual removal effects. There is no Rust
decision fallback when a compiled Mojo ABI call fails.

The hot path passes only scalar totals to the compiled trim planner. It
materializes at most 65 path sequence numbers *only after* Mojo decides
global eviction is necessary; it does not collect all entries or scan paths
for every append. Publicly exposed constant limits remain part of the
existing Rust API, not alternative eviction authority.

## Verification

- Compiled Mojo live-log module and strict core ABI tests: 2 new direct
  Rust tests plus direct Mojo assertions pass.
- All 28 prodex-runtime-log tests pass, including the million-event
  bounded-memory test and new tests for per-path byte limits, global
  entry limits, sequence ties, path-key iteration order and re-added paths.
- ABI negative tests validate unsupported version, invalid scope,
  invalid pointers and unchanged output for rejected arguments.
- Mojo no-fallback self-test, formatting, diff validation and canonical
  production-share non-regression check pass.

The official production share increases from 35.75917369898641% to
35.76207093143992% (129,846 Mojo / 233,237 Rust).
The 75% project target remains open, and the project metric definitions
are unchanged. Remote CI is not a prerequisite for this checkpoint.
