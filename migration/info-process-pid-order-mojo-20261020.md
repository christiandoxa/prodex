# Info Process PID Order Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled info_render.mojo owner now chooses the sorted
original source indices for process rows by ascending u32 PID
through the versioned prodex_info_process_pid_order_v1 ABI.
Original row order is preserved for identical PID values.

Rust retains /proc and ps process discovery, process-name
classification, the original ProdexProcessInfo records, and
application of validated Mojo-selected indices. The previous
Rust processes.sort_by_key PID policy was deleted without fallback.

## Verification

- All 23 prodex-app reports::info tests pass.
- Two new real application caller regressions pass for duplicate
  PID rows with different commands/runtime flags, u32::MAX,
  PID zero and 1,025 records.
- Two compiled-core ABI tests pass for stable numeric order,
  malformed later PID values, invalid versions/capacity and
  non-mutating rejected calls.
- Direct Mojo ABI test, anti-fallback guard/self-test, Cargo
  formatting and official production-share non-regression pass.

Official broad share rises from 36.05829134839374% to
36.06472593559358% (133,191 Mojo / 236,120 Rust production LOC).
The 75% target remains unmet; historical counting and release
floors remain unchanged. Remote CI green is not a checkpoint
publication prerequisite.
