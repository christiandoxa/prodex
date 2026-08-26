# Rust and Mojo equivalents

This is a semantic mapping for current Prodex usage, not a syntax translation guide.
The quota, runtime scoring, provider routing/capability, and byte-estimation rows are enabled
in production code behind the opt-in `mojo-core` feature; Mojo is authoritative when enabled,
while Rust-only builds remain separately supported without that feature.

| Rust construct / usage | Current Prodex location | Mojo equivalent verified or expected | Confidence | Complexity | External dependencies | Candidate | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `i64` arithmetic and bounded branches | `prodex-quota::render::remaining_percent` | `Int64`, `def`, `if`, C ABI scalar return | Verified | Low | None | `MOVE_NOW` | Compiled by local Mojo 1.0.0 and called from Rust |
| `Option<i64>` at an FFI edge | Same function | Explicit `(value, has_value)` scalar pair | Verified | Low | None | `MOVE_NOW` | Avoids exposing allocator-owned or tagged heap data |
| quota window thresholds | `prodex-quota::render::quota_window_summary` | Explicit status tag plus scalar remaining percent | Verified | Low | None | `MOVE_NOW` | Missing-window state stays in Rust; Mojo receives `has_window` |
| quota pressure aggregation | `prodex-quota::render::quota_pressure_band_from_windows` | Two status tags and one band tag | Verified | Low | None | `MOVE_NOW` | Mojo applies the same ordered max mapping |
| quota pair eligibility | `prodex-quota::render::window_pair_has_ready_limit` | Four scalar values plus presence flags | Verified | Low | None | `MOVE_NOW` | One batch call covers both windows |
| route-specific quota pressure | `prodex-runtime-proxy::runtime_proxy_quota_pressure_band_for_route` | Five scalar inputs and one band tag | Verified | Medium | None | `MOVE_NOW` | Rust builds observations; Mojo applies route thresholds in one call |
| bounded profile scheduling | `prodex-runtime-quota::selection::schedule_ready_profile_candidates_with_view` | Flat 16-field `Int64` rows and stable output indices | Verified | High | None after normalization | `MOVE_NOW` | Maximum 256 records; Mojo derives score, reserve bias, hysteresis, and order while Rust-only builds keep their separate implementation |
| provider routing plan | `prodex-provider-spi::governed_routing::plan_governed_provider_route` | Flat candidate arrays plus eligibility, score, and stable-order outputs | Verified | High | None after Rust normalization | `MOVE_NOW` | Mojo owns capability filtering, score arithmetic, and stable eligible ordering; Rust owns hard gates, affinity constraints, route construction, and policy |
| provider capability matching | `prodex-provider-spi::negotiate_provider_route_capability` | Flat well-formed flags and capability masks plus tagged outputs | Verified | Medium | None after Rust normalization | `MOVE_NOW` | Mojo returns compatibility and first-index facts; Rust owns route/model objects and diagnostics |
| Smart Context byte estimate | `prodex-runtime-proxy::smart_context::token_accounting::smart_context_estimate_tokens_from_body_bytes` | `UInt64` bytes to saturated `UInt64` estimate | Verified | Low | None | `MOVE_NOW` | Text tokenization remains Rust |
| `u64` checked/saturating arithmetic | `prodex-domain::accounting` | `UInt64` plus explicit overflow branches | Unverified | Low | None | `KEEP_RUST` | Generic accounting helper has no production Mojo seam; durable accounting remains Rust-owned |
| fixed-layout Rust structs | context text ABI DTOs | Mojo `struct` with pointer/native-length or fixed integer fields | Verified | Medium | None | `MOVE_NOW` for versioned DTOs | Rust static assertions and Mojo reflection/runtime layout probes are mandatory |
| Rust enums / tagged decisions | domain and provider plans | Mojo enum-like tagged representation or explicit integer tag | Unverified | Medium | None | `EXPERIMENT` | Keep Rust enum authoritative at first |
| `Vec` / `BTreeMap` collection algorithms | critical-signal duplicate grouping | Mojo `InlineArray` plus caller-owned bounded hash buffers | Verified | Medium | None | `MOVE_NOW` | One batch call; owning `List`/`Dict`/`Set` remain runtime-dependent experiments |
| sorting and ranking | `prodex-runtime-proxy::selection_plan`, Smart Context | Mojo collections and comparator logic | Unverified | Medium/high | token/input adapters | `EXPERIMENT` | Candidate only after hard-affinity extraction |
| borrowed UTF-8 comparison/grouping | `prodex-context::critical_signal_normalized_rows` | validated `StringSlice`, exact comparison, versioned string-view records | Verified | Medium | None | `MOVE_NOW` | Rust retains ANSI/line normalization, Unicode trim, secrets, and user-facing errors |
| owning `String` normalization | future token/policy helpers | Mojo `String` / string methods | Verified with runtime dependency | Medium | compiler runtime bundle | `EXPERIMENT` | Current static release deliberately avoids heap-owning Mojo values |
| `Result`/error plans | context text ABI | Explicit status plus structured capacity/result record | Verified | Medium | None | `MOVE_NOW` | Rust maps tags to `MojoError`; no heap error crosses FFI |
| JSON/TOML parsing and serialization | config, provider, quota IO | Mojo APIs exist in ecosystem but not verified for Prodex | Low | High | schema/compatibility | `KEEP_RUST` | Serde/TOML remain the boundary oracle |
| async/network/TLS/process/filesystem | app, gateway, runtime, storage | No clean equivalent used by this slice | High | High | mature Rust crates | `KEEP_RUST` | Do not recreate Tokio, Hyper, Reqwest, rustls, or OS APIs |

## Mapping rule

Move a complete deterministic calculation, not a Rust method because its syntax looks portable.
Rust may pass validated text views and structured records when ownership and confidentiality are
explicit; numeric pre-normalization is no longer universal. Captured vectors and invariants remain
the compatibility contract after parity covers normal, Unicode, boundary, invalid, and extreme
inputs. A separate Rust-only build may retain its implementation, but it is never a runtime
fallback for `MOJO`.

## Rich equivalents promoted on 2026-08-26

| Rust usage | Mojo domain equivalent | Boundary strategy | Status |
| --- | --- | --- | --- |
| `prodex-context` line classification/grouping | `DiagnosticRecord`, Unicode-aware scanner, bounded key table, `ContextAnalysis` | borrowed `RichStringView` plus output `RichSlice` arena | `MOJO` |
| provider fallback aliases and `combo:` chains | `NormalizedIdentifier`, bounded model records, case-folded dedup set | provider/model text views plus ordered output records | `MOJO` |
| gateway route-alias validation | `PolicyRule`, optional strategy view, metric relationship checks, `MojoIssue` | policy record table plus structured issue fields | `MOJO` |
| governed provider routing | `RouteCandidate`, capability tokenization, score components, stable rank | non-secret provider/model/capability views plus route record graph | `MOJO` |
| Smart Context rehydration | `ContextItem`, available-reference set, `ContextPlan` and action records | opaque artifact-reference views plus caller-owned output | `MOJO` |
| JSON/TOML/provider wire parsing | no direct replacement | Serde and protocol adapters remain Rust | `RUST` |

The release implementation uses arena-backed equivalents rather than owning `List`/`Dict`/`Set`.
This keeps collection processing in Mojo while preserving the existing static archive, GLIBC, and
clean-machine contracts.
