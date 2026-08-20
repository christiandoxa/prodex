# Rust-Mojo FFI contract

## Current boundary

Rust calls the quota policy exports when the `prodex-quota/mojo` Cargo feature is enabled:

```text
prodex_quota_remaining_percent(used_percent: Int64, has_value: Int64) -> Int64
prodex_quota_window_status(remaining_percent: Int64, has_window: Int64) -> Int64
prodex_quota_pressure_band(five_hour_status: Int64, weekly_status: Int64) -> Int64
prodex_quota_window_pair_has_ready_limit(
    first_used_percent: Int64,
    first_has_value: Int64,
    second_used_percent: Int64,
    second_has_value: Int64,
) -> Int64
```

The `prodex-provider-spi/mojo` feature uses the shared `mojo-routing` source for one complete
bounded routing-plan batch and one capability-match batch:

```text
prodex_routing_plan_batch(
    hard_eligible: *const Int64,
    capability_masks: *const Int64,
    provider_order: *const Int64,
    health: *const Int64,
    load: *const Int64,
    quota_headroom: *const Int64,
    quota_present: *const Int64,
    cost: *const Int64,
    latency: *const Int64,
    risk: *const Int64,
    priority: *const Int64,
    affinity: *const Int64,
    eligible: *mut Int64,
    reason_tags: *mut Int64,
    normalized_values: *mut Int64,
    weighted_totals: *mut Int64,
    scores: *mut Int64,
    ordered_indices: *mut Int64,
    ordered_count: *mut Int64,
    count: Int64,
    required_capability_mask: Int64,
    health_weight: Int64,
    load_weight: Int64,
    cost_weight: Int64,
    latency_weight: Int64,
    risk_weight: Int64,
    priority_weight: Int64,
    affinity_weight: Int64,
) -> Int64

prodex_capability_match_batch(
    well_formed: *const Int64,
    capability_masks: *const Int64,
    compatible: *mut Int64,
    reason_tags: *mut Int64,
    first_compatible: *mut Int64,
    first_incompatible: *mut Int64,
    count: Int64,
    required_capability_mask: Int64,
) -> Int64
```

The symbols use the platform C ABI. `has_value` is `0` for Rust `None` and `1` for
Rust `Some(_)`; the `used_percent` value is ignored when `has_value == 0`.

The policy entry points use explicit integer tags:

| Concept | Codes |
| --- | --- |
| Window status | `Ready=0`, `Thin=1`, `Critical=2`, `Exhausted=3`, `Unknown=4` |
| Pressure band | `Healthy=0`, `Thin=1`, `Critical=2`, `Exhausted=3`, `Unknown=4` |
| Boolean result | `0=false`, `1=true` |
| Routing-plan reason | `Eligible=0`, `HardRejected=1`, `CapabilityMissing=2` |
| Capability-match reason | `Malformed=0`, `Compatible=1`, `Missing=2` |
| Core ABI version | `1` |

## Ownership and lifetime

| Concern | Contract |
| --- | --- |
| Allocation | Rust owns flat input/output vectors; Mojo allocates nothing across the ABI. |
| Owner | Rust owns the logical input; Mojo owns no Rust memory. |
| Mutation | Neither side mutates shared memory. |
| Freeing | No allocation means neither side frees anything. |
| Lifetime | Values live only for the synchronous call. |
| Thread safety | Function is stateless and reentrant; no global state. |
| Errors | No error path exists for bounded integer arithmetic; invalid ranges are clamped exactly like Rust. |
| Secrets | No secret, token, prompt, path, or user string crosses the boundary. |

## Behavioral contract

```text
None       => 0
Some(x<0)  => 100
Some(0..100) => 100-x
Some(x>100) => 0
```

For an existing quota window, status is derived from remaining percent: `0` is
`Exhausted`, `1..=5` is `Critical`, `6..=15` is `Thin`, and `16..=100` is `Ready`.
An absent window is `Unknown`. The pressure band is the maximum mapped band of the
five-hour and weekly statuses. Window-pair readiness is false when both values are
missing or either present value is at least `100`.

Rust retains the original implementation as the default path and as the differential
oracle. Mojo is opt-in because Cargo builds must remain usable on machines without the
Mojo compiler. If the opt-in feature finds no compiler or archiver on `PATH`, Cargo emits
an explicit warning and uses the Rust implementation; an explicitly configured but
failing tool or a Mojo compile error fails the build.

## Build contract

`crates/prodex-mojo-core/build.rs` is the single build script. It:

1. runs only when a `prodex-mojo-core` feature is enabled;
2. invokes the current `mojo build --emit object --optimization-level=3` for each selected source;
3. archives the objects as one `libprodex_mojo_core.a`;
4. forwards that static archive to final Cargo link targets;
5. accepts `PRODEX_MOJO`, `AR`, and `PRODEX_MOJO_ARCHIVE` only as local build overrides;
6. emits `prodex_mojo_active` only after the current source archive is ready;
7. treats `PRODEX_MOJO_REQUIRED=1` as strict mode: a missing compiler/archiver, missing
   prebuilt archive, or failed build is a hard error;
8. never downloads tools or invokes network access.

Generated objects and archives are never committed. Release cross-linking builds a target
archive outside the final Cargo output directory, then sets `PRODEX_MOJO_ARCHIVE` so the Rust
target linker consumes that exact archive. Other target rows remain Rust-only until final-link,
runtime, and deployment evidence exists.

The Rust-only default and optional local Mojo build may still use the Rust fallback when
the compiler is not installed. The dedicated real-Mojo CI lane sets
`PRODEX_MOJO_REQUIRED=1`, so fallback is not accepted as coverage.

## Runtime-proxy route policy boundary

`prodex-runtime-proxy/mojo` is enabled through the application `mojo-quota` feature and
uses a separate stateless object containing:

```text
prodex_runtime_quota_pressure_band_for_route(
    five_hour_remaining_percent: Int64,
    five_hour_has_value: Int64,
    weekly_remaining_percent: Int64,
    weekly_has_value: Int64,
    route_kind: Int64,
) -> Int64
```

Route tags are `Responses=0`, `Compact=1`, `Websocket=2`, and `Standard=3`. The result
uses the pressure-band tags above. Rust still owns observation construction, route enums,
fallback execution, and all transport/affinity behavior.

## Shared core and batch boundaries

`prodex-mojo-core` is the only crate that declares unsafe FFI symbols. Consumer crates call safe
Rust wrappers and never declare a Mojo symbol themselves. The shared static archive currently
contains these additional deterministic entry points:

```text
prodex_mojo_abi_version() -> Int64
prodex_runtime_quota_profile_score_batch(..., count: Int64) -> Int64
prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes: UInt64) -> UInt64
prodex_routing_score_batch(..., count: Int64, weights...) -> Int64
prodex_routing_plan_batch(..., count: Int64, required_capability_mask: Int64, weights...) -> Int64
prodex_capability_match_batch(..., count: Int64, required_capability_mask: Int64) -> Int64
prodex_smart_context_pressure_snapshot(..., output pointers...) -> Int64
prodex_runtime_candidate_plan_batch(..., count: Int64, route_kind: Int64) -> Int64
```

Existing routing and capability batches use parallel flat `Int64` arrays and accept at most 64
records. The runtime candidate-plan batch uses the same layout and accepts at most 256 records.
A zero-length batch is valid and does not read or write per-record arrays;
the scalar result pointers (`ordered_count`, `first_compatible`, and `first_incompatible`) still
need one writable slot. Non-zero batches require every per-record pointer to reference at least
`count` elements; null non-zero pointers are invalid and must never be passed. Rust keeps all
vectors alive for the synchronous call, and Mojo does not retain any pointer after return.

The routing-score batch writes seven normalized components, one weighted total, and one final
score per record. The complete routing-plan batch additionally writes one eligibility flag and
one reason tag per input, plus an eligible-only ordered-index array and its count. It computes
scores for every input, but Rust uses scores and ordered indices only for eligible routes. The
capability-match batch writes one compatibility flag and reason tag per input plus the first
compatible and first well-formed incompatible indices. The runtime quota batch writes four
`Int64` values per record. The Rust wrappers validate statuses, bounds, indices, and integer
conversions before exposing results to callers.

For routing-plan ordering, Mojo compares eligible candidates by affinity descending, score
descending, provider order ascending, then original candidate index ascending. Rust maps provider
identities to the bounded provider-order integer and keeps route construction, candidate
evaluation details, policy, credentials, and affinity/error handling outside the ABI. For
capability matching, malformed route/model tokens are marked `Malformed` and skipped when
choosing first indices; a well-formed candidate is compatible when its mask contains the
required mask.

| Concern | Batch contract |
| --- | --- |
| Allocation | Rust allocates all flat input/output vectors; Mojo allocates nothing across the ABI |
| Ownership | Rust owns and may mutate output buffers; Mojo only reads inputs and writes outputs during the call |
| Lifetime | Pointer lifetime is one synchronous call; no pointer escapes |
| Alignment | Rust slices provide native `i64` alignment; Mojo pointers are typed `Int64` pointers |
| Thread safety | Functions are stateless and reentrant; concurrent calls use disjoint caller-owned buffers |
| Nullability | Non-zero per-record buffers must be valid; zero-count per-record buffers are not read or written, but scalar result pointers still need one writable slot |
| Error propagation | Explicit integer status; no panic, exception, Rust enum, `Vec`, `String`, or `Result` crosses the boundary |
| Fallback | Build-time missing Mojo selects the documented Rust fallback; strict builds fail. Capability negotiation falls back to Rust if its Mojo result is invalid; governed routing rejects an invalid plan result |

## Runtime candidate-plan and Smart Context pressure contracts

`prodex_smart_context_pressure_snapshot` is a synchronous fixed-width decision call. Rust passes
an optional model window as `model_context_window_tokens` plus a presence flag, effective input
tokens, reserved output tokens, an input-source tag, and three normalized risk flags. Source tags
are `CurrentRequestTokens=0`, `CurrentRequestBodyEstimate=1`, `ObservedHistory=2`, and
`Unknown=3`. Risk flags are `0` or `1`.

The caller-owned outputs are effective usable tokens plus a presence flag, pressure basis points
plus a presence flag, pressure-band tag, absolute safety floor, and estimator-confidence tag.
Pressure tags are `Unknown=0`, `Low=1`, `Moderate=2`, `High=3`, `Critical=4`, and
`Exhausted=5`. Confidence tags are `High=0`, `Medium=1`, and `Low=2`. Status `0` means success;
status `1` means an invalid normalized tag. Rust rejects any unexpected status, presence flag,
band, confidence, or numeric conversion before mapping the result to the public Smart Context
model. The result is used by `smart_context_observed_token_accounting_with_calibration`;
token estimation, model lookup, risk discovery, and available-context construction remain Rust.

`prodex_runtime_candidate_plan_batch` receives one flat row per already-normalized candidate.
Each row has 22 signed `Int64` fields:

```text
0  selection-backoff flag
1  provider priority
2..10 quota sort key (fields 5..7 are descending components)
11 quota source tag
12 inflight count
13 health sort key
14 prompt-cache affinity rank
15 encoded prompt-cache affinity key
16 original order index
17 encoded jitter
18..21 backoff sort key
```

Route tags are `Responses=0`, `Compact=1`, `Websocket=2`, and `Standard=3`. The output arrays
are caller-owned ready and fallback indices plus their counts. Ready excludes candidates marked
with selection backoff. Fallback contains every input candidate, so the two output arrays may
overlap; Rust validates that each array is duplicate-free, in bounds, and that fallback covers
the complete input set. Status `0` means success, `1` means an invalid count, and `2` means an
invalid route or normalized tag. The maximum count is 256; zero candidates are valid.

Rust encodes `u64` ordering fields as the bit pattern of `value ^ (1 << 63)` in an `i64`, which
preserves unsigned ascending order when Mojo compares signed values. No Rust object layout,
string, collection, enum, or affinity state crosses the ABI. Rust filters excluded profiles,
acquires health/quota state, constructs hard quota guards, computes prompt-cache ownership keys,
and reconstructs the public plan. Mojo owns only the deterministic bounded ordering.

The shared ABI version remains `1`: these are additive symbols using the established static
archive and do not change existing entry-point layouts.

## Shared build contract

`crates/prodex-mojo-core/build.rs` compiles the feature-selected current Mojo sources into one
`libprodex_mojo_core.a`. It passes the Cargo target triple and an explicit safe target CPU to
`mojo build --emit object --optimization-level=3`. Release cross-linking may set
`PRODEX_MOJO_ARCHIVE` to a separately generated target archive; the archive is copied into the
Cargo output directory and is still linked statically. The archive path must not be the Cargo
output archive itself.

The supported strict controls are:

```text
PRODEX_MOJO_REQUIRED=1
PRODEX_MOJO_VERSION=1.0.0
PRODEX_MOJO_TARGET=<Cargo target triple>
PRODEX_MOJO_TARGET_CPU=<verified Mojo CPU name>
PRODEX_MOJO_ARCHIVE=<verified target archive, release cross-link only>
```

Relative archive paths resolve from the workspace root so the same path works on the host and
inside the `cross` build container. Strict release builds forward the Mojo variables through
`CROSS_CONTAINER_OPTS`.

Runtime diagnostics intentionally report `compiler_required=false`: the compiler is never a
runtime dependency. `build_strict=true` means `PRODEX_MOJO_REQUIRED=1` governed the build and
missing compiler/archiver/archive failed it. `compiler_required` and `build_strict` therefore
describe different phases.

No generated object or archive is committed. The final Linux release is verified for static
Mojo dependencies, no build-path RPATH/RUNPATH, GLIBC policy, and execution without `mojo` on
`PATH`.
