# Runtime quota summary ordering uses Mojo authority

`prodex_runtime_quota_summary_sort_key_v1` now owns the deterministic ordering
of persisted quota summaries used for fresh cold-start probe selection. It
validates the ABI inputs, ranks route and window status tags, computes the
minimum remaining reserve, and preserves remaining-percentage and reset-time
tie-break fields.

Rust still acquires persisted summaries, maps validated tags to runtime enums,
and materializes the `Reverse` sort tuple. The previous Rust rank, minimum,
and tie-break calculations were removed. Required-Mojo ABI and runtime-proxy
caller regressions cover the boundary values and ordering contract; no
production fallback remains.

## Validation

- Required-Mojo ABI test: 1 passed; adapter unit cases: 3 passed; runtime-proxy
  caller case: 1 passed; full runtime-proxy library: 440 passed.
- `cargo fmt --check`, package Clippy with `--all-features --all-targets`,
  ownership, authority, no-fallback, production-share, and `git diff --check`:
  passed.
- Broad production share: 33.785346% (114,558 / 339,076 LOC); the 7% release
  floor passes and the 75% project target remains outstanding.
