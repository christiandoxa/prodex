# Rust and Mojo equivalents

This is a semantic mapping for current Prodex usage, not a syntax translation guide.
Only the first row is enabled in production code behind an opt-in Cargo feature.

| Rust construct / usage | Current Prodex location | Mojo equivalent verified or expected | Confidence | Complexity | External dependencies | Candidate | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `i64` arithmetic and bounded branches | `prodex-quota::render::remaining_percent` | `Int64`, `def`, `if`, C ABI scalar return | Verified | Low | None | `MOVE_NOW` | Compiled by local Mojo 1.0.0 and called from Rust |
| `Option<i64>` at an FFI edge | Same function | Explicit `(value, has_value)` scalar pair | Verified | Low | None | `MOVE_NOW` | Avoids exposing allocator-owned or tagged heap data |
| `u64` checked/saturating arithmetic | `prodex-domain::accounting` | `UInt64` plus explicit overflow branches | Unverified | Low | None | `EXPERIMENT` | Compile a representative pair before migration |
| small Rust structs | domain and quota models | Mojo `struct` with scalar fields | Documented, unverified here | Low/medium | None | `EXPERIMENT` | Do not expose these structs across FFI until layout is tested |
| Rust enums / tagged decisions | domain and provider plans | Mojo enum-like tagged representation or explicit integer tag | Unverified | Medium | None | `EXPERIMENT` | Keep Rust enum authoritative at first |
| `Vec` / `BTreeMap` collection algorithms | quota pool and routing helpers | Mojo `List` / `Dict` or flat buffers | Unverified | Medium | stdlib only | `EXPERIMENT` | Prefer one batch call, not per-element FFI |
| sorting and ranking | `prodex-runtime-proxy::selection_plan`, Smart Context | Mojo collections and comparator logic | Unverified | Medium/high | token/input adapters | `EXPERIMENT` | Candidate only after hard-affinity extraction |
| `String` normalization | profile identity and policy helpers | Mojo `String` / string methods | Unverified | Low/medium | None | `EXPERIMENT` | Keep secrets and user-facing errors in Rust |
| `Result`/error plans | domain/application plans | Explicit tagged output or scalar status | Unverified | Medium | None | `REFACTOR_THEN_MOVE` | No implicit panic/error crossing FFI |
| JSON/TOML parsing and serialization | config, provider, quota IO | Mojo APIs exist in ecosystem but not verified for Prodex | Low | High | schema/compatibility | `KEEP_RUST` | Serde/TOML remain the boundary oracle |
| async/network/TLS/process/filesystem | app, gateway, runtime, storage | No clean equivalent used by this slice | High | High | mature Rust crates | `KEEP_RUST` | Do not recreate Tokio, Hyper, Reqwest, rustls, or OS APIs |

## Mapping rule

Move a complete deterministic calculation, not a Rust method because its syntax looks
portable. Inputs must be normalized by Rust, output must be a small explicit value, and
the Rust implementation must remain available as a differential oracle until parity has
survived normal, boundary, invalid, and extreme inputs.
