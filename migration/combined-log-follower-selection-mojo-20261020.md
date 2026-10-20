# Combined Runtime/Session Log-Follower Selection Migrated to Mojo (2026-10-20)

## Production ownership

The production log_semantics.mojo owner now chooses a shared
descriptor-bounded runtime/session log follower set through the
versioned prodex_mojo_log_follow_selection_v1 ABI. The kernel
performs full modification-timestamp sorting (including pre-epoch
times and nanoseconds), raw path-byte ascending tie-breaks,
runtime-before-session tie resolution, and chronological
candidate reservation. With both sources present, it reserves
the historical 16/16 minimum shares within the 32-file budget
and then fills unused slots from the newest remaining candidates.

Exact source/path duplicates count against the descriptor cap
only once but remain present in the caller's original path
sequence if the source/path pair is selected. Fewer than or
equal to the requested cap retains every original row. The
compiled selection path validates all borrowed metadata before
writing output, uses bounded host-owned scratch arrays, and
retains only the selected representative indices in a small
bounded stack buffer.

Rust retains filesystem metadata acquisition, PathBuf ownership,
the original input order and the I/O of selected logs. The old
Rust sort_by, BTreeSet uniqueness, source reservation counters
and fill loop were removed without a fallback copy of that policy.

## Verification

- All 127 strict prodex-app log module tests pass, including new
  real caller tests with 40 runtime and 40 session files,
  modification-time ordering, ten-session underfill, duplicates,
  and unchanged original path ordering.
- Three direct core ABI tests pass for source reservations,
  uniqueness, empty/max-bound inputs, malformed source/length
  and unchanged outputs on rejected requests.
- Direct Mojo ABI test, no-fallback guard/self-test, Cargo fmt,
  git diff check and canonical production-share non-regression
  validations pass.

The canonical broad Mojo production share rose from
35.921607541552966% to 35.95155464268008%
(131,976 Mojo / 235,118 Rust production LOC).
The 75% project target is still unmet. Counting definitions
and historical release floors remain unchanged, and remote
CI green was not a checkpoint prerequisite.
