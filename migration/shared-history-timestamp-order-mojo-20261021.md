# Shared Codex History Chronological Merge Ordering Migrated to Mojo (2026-10-21)

## Compiled production ownership

The existing compiled shared_history_policy.mojo owner now chooses the
output permutation of original history.jsonl records through the versioned
prodex_shared_history_timestamp_order_v1 ABI. Dedupe and byte budgeting
were already compiled Mojo policies.

The chronological sorter processes consecutive timestamped records in
ascending signed i64 timestamp order, with stable original-position
priority for equal timestamps. Records with absent or noninteger ts
retain their original position and divide timestamped runs. This
preserves the original Rust comparator's rule: when both rows have
timestamps, compare those timestamps; otherwise preserve original
input order. The Mojo algorithm uses bounded iterative heap sorting
on host-owned index buffers, rather than a Rust comparator fallback.

Rust retains the history file safety checks, no-follow file open,
bounded JSONL reads, Serde timestamp acquisition, original strings,
permission preservation and atomic write. The previous Rust sort_by
comparator and redundant HistoryLine.order field have been deleted.
The existing Mojo dedup planner still runs before ordering.

## Verification

- All 94 prodex-shared-codex-fs tests pass. New real filesystem
  regressions validate merging source/destination histories, first
  duplicate retention, missing and invalid timestamp barriers,
  negative/extreme signed timestamps, stable equal timestamps,
  output idempotence and original file semantics.
- Two direct prodex-mojo-core ABI tests verify mixed valid/missing
  timestamps, stable ties, more than 1,000 observations, invalid
  tags/capacity/version, and no output mutation on invalid calls.
- A direct Mojo ABI regression, source no-fallback guard/self-test,
  Cargo formatting and canonical production-share non-regression
  checks pass.

Canonical production share rises from 36.102403908513125% to
36.11279186610626% (133,691 Mojo / 236,513 Rust production LOC).
The 75% long-term project goal remains unmet. Accounting, the
historical release floor and the no-fallback policy were not weakened;
a green remote CI run was not a prerequisite to this checkpoint.
