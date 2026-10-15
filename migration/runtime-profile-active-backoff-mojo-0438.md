# Active profile and route backoff reduction migrated to Mojo

`prodex_runtime_profile_active_backoff_max_v1` is now the production
semantic owner for filtering expired retry, transport, and route-circuit
deadlines and selecting the latest still-active timestamp. The kernel
supports up to one million caller-owned timestamp values and validates
its version, pointers and count before committing the output.

The production consumer
`crates/prodex-runtime-proxy/src/health/backoff.rs` now only identifies
profile-specific keys, retrieves timestamps from Rust-owned BTreeMaps,
and materializes a bounded Mojo input slice. Rust implementations of
active-window comparison and maximum/union selection were replaced with
the compiled Mojo reducer; there is no Rust fallback selection algorithm.
Existing health score/backoff sort plan remains Mojo-owned.

## Required-Mojo local evidence

- Compiled Mojo ABI tests: 2 passed for negative and maximum i64 values,
  exact-expiry behavior, empty input, and rejected ABI counts/pointers.
- Production caller test verifies route vs global backoffs, profile-key
  isolation, retry/transport/circuit union, and strict expiration.
- Full required-Mojo runtime-proxy library suite: 435 passed.
- Clippy with `--all-targets -- -D warnings`: passed.
- Ownership, authority, no-fallback, production-share guards: PASS.
- Broad production source at this checkpoint: 33.42% Mojo
  (111,990 Mojo LOC / 335,096 total LOC).
  The 75% project target remains unmet.
