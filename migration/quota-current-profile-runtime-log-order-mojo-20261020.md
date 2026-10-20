# Runtime Quota Current-Profile Log Ordering Migrated to Mojo (2026-10-20)

## Production ownership

The compiled quota.mojo owner now chooses the chronological scan order
of runtime log candidates that determine the quota current profile,
through versioned prodex_quota_runtime_log_order_v1. The kernel
preserves the exact former Rust Option<SystemTime> ordering: missing
modified-time observations first, then older signed timestamps,
nanosecond precision, raw-encoded path bytes ascending on ties and
stable input index on exact equality.

Rust retains log file and SystemTime metadata acquisition, borrowed
path ownership, each log read, Serde/message parsing and validation
that an observed profile still exists in the current application
state. The previous Rust paths.sort_by and repeated metadata lookups
within comparisons have been removed. No Rust sort-policy fallback
remains.

## Verification

- All 14 strict quota_support application tests pass, including two
  new actual-file regressions for newest recognized profile
  selection, unknown/deleted profiles, missing metadata, and
  Unicode path tie-breaking on equal modification times.
- Two direct compiled-core ABI tests cover pre-epoch and nanos,
  absent timestamps, Unicode path names, 1,025 input rows,
  malformed late row values, wrong versions/capacity, and
  untouched outputs on invalid requests.
- Direct Mojo ABI regression, no-fallback guard/self-test, Cargo
  formatting, diff checks and official production share
  non-regression checks pass.

Official share rises from 35.98176843960252% to
35.996325158260106% (132,433 Mojo / 235,474 Rust production LOC).
The separate 75% project target remains unmet. No metric
definitions or historical release floors changed, and green
remote CI was not a prerequisite for the checkpoint.
