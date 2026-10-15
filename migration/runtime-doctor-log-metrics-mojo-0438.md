# Runtime-doctor log metric decoding and precedence

The six persistence/probe summary backlog and latency fields now use
`prodex_mojo_runtime_doctor_log_metric_plan_v1` in the compiled
`runtime_doctor_marker_summary.mojo` domain owner.

The kernel takes thirteen borrowed marker fields, parses unsigned decimal
values (including an optional leading `+`), rejects invalid characters
and overflows, applies the host `usize` bound only to backlog metrics,
and selects the first valid candidate within each ordered source group.
The previous Rust `or_else` chains and unused `u64` parser are deleted.
Rust retains only marker-map lookup, FFI marshalling, bound-checked value
projection and DTO assignment; it does not fall back to Rust parsing.

The production caller is
`crates/prodex-runtime-doctor/src/diagnosis/final_summary/log_summary.rs`.
The operation and adapter are declared in `migration/mojo-ownership.json`.
There is no duplicate Rust production policy owner.

## Checkpoint evidence

- Required-Mojo direct metric tests: 3 passed, covering parsing,
  overflow, `usize` limits, precedence, invalid ABI, flags and buffers.
- Required-Mojo runtime-doctor caller tests: 2 passed.
- Full required-Mojo runtime-doctor library suite: 54 passed.
- Clippy with `--all-targets -- -D warnings`: passed.
- Mojo ownership, authority, no-fallback and production-share guards:
  all passed.
- Production-source Mojo share at this checkpoint: 33.15%
  (110,416 Mojo LOC of 333,070 total); the 75% target remains pending.
