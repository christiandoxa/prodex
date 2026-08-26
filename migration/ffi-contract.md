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
| Secrets | No secret, token, prompt, path, or credential-bearing string crosses the numeric boundary. The text boundary below is restricted to non-secret diagnostic lines. |

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

Rust-only release targets retain their separately compiled implementation. Mojo is opt-in
because Cargo builds must remain usable on supported targets without a Mojo toolchain. Once
a Mojo feature is enabled, a missing compiler, archiver, or prebuilt archive fails the build;
there is no feature-enabled Rust fallback.

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

Rust-only builds omit Mojo features. Once a Mojo feature is enabled, the compiler and archive
are mandatory; a missing tool fails the build and invalid Mojo output fails the call without
recomputing in Rust.

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
prodex_runtime_quota_profile_schedule_batch(..., count: Int64) -> Int64
prodex_runtime_policy_validate_numeric(..., count: Int64) -> Int64
prodex_context_signal_diff(..., seven counters) -> Int64
prodex_mojo_text_abi_version() -> Int64
prodex_mojo_text_abi_layout(output: *mut UInt64, output_count: Int64) -> Int64
prodex_context_prepare_signal_rows_v1(...string-view records and caller buffers...) -> Int64
prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes: UInt64) -> UInt64
prodex_routing_plan_batch(..., count: Int64, required_capability_mask: Int64, weights...) -> Int64
prodex_capability_match_batch(..., count: Int64, required_capability_mask: Int64) -> Int64
prodex_smart_context_pressure_snapshot(..., output pointers...) -> Int64
prodex_runtime_candidate_plan_batch(..., count: Int64, route_kind: Int64) -> Int64
```

Existing routing and capability batches use parallel flat `Int64` arrays and accept at most 64
records. The runtime candidate-plan batch uses the same layout and accepts at most 256 records.
The routing-plan implementation retains its score sub-batch as an internal Mojo function; it is
not a separately exported or Rust-callable ABI.
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

The profile-schedule batch accepts up to 256 rows of 16 signed fields. Rust gathers provider
priority, cooldown state, last-selection time, raw profile pressure fields, quota source,
preferred flags, and original order. Mojo derives the complete score, reserve bias,
preferred-profile hysteresis, and stable permutation; Rust retains clock/state reads, profile
names, and persisted preference state. The numeric policy batch accepts a
section-sized list of `NonZero=0`, `Range=1`, and `LessOrEqual=2` rules. Mojo writes one `Int64`
failure flag per input rule; Rust validates the flags and maps failed indices to existing
path-aware errors.

The legacy context signal-diff batch exchanges exactly seven non-negative `Int64` counters for
each side and writes seven lost plus seven gained counters. The additive text ABI below now owns
duplicate-line grouping and row-plan construction; Rust still strips terminal escapes,
normalizes line endings, performs Unicode-compatible trimming, and classifies diagnostic lines.

## Context text ABI version 1

`prodex-context/mojo` passes actual non-secret UTF-8 line text through a language-neutral record:

```c
struct ProdexStringView {
    const uint8_t *ptr;
    size_t len;
};

struct ProdexBytesView {
    const uint8_t *ptr;
    size_t len;
};
```

Rust declares the record with `#[repr(C)]`. Mojo represents the nullable pointer as
`Optional[Pointer[UInt8]]` and the length as native `UInt`. On every supported 64-bit target the
expected size/alignment is `16/8`, with offsets `0/8`. Rust compile-time assertions and
`prodex_mojo_text_abi_layout` compare Rust and Mojo sizes, alignments, and field offsets at
runtime before promotion tests pass. `ProdexBytesView` has the same proven layout but carries no
UTF-8 promise; the production context operation uses `ProdexStringView` and validates it before
text interpretation.

The production call is one coarse-grained operation:

```text
prodex_context_prepare_signal_rows_v1(
    abi_version,
    before StringView records + seven counters per line,
    after StringView records + seven counters per line,
    caller-owned row/result/scratch buffers,
) -> status
```

Mojo validates UTF-8 before constructing zero-copy `StringSlice` values. Validation rejects
truncated sequences, overlong encodings, surrogate code points, and values above `U+10FFFF`;
embedded nul bytes are ordinary data. Empty views may use null or non-null pointers. Non-empty
views require a non-null pointer valid for exactly `len` readable bytes; no sentinel byte is read.
Each length must fit `Int64`, each side is limited to 65,536 lines, unique keys are limited to
65,536, and the open-addressed scratch table is at most 131,072 slots.

Mojo uses `InlineArray[Int64, 7]` for typed line counters and caller-owned buffers for the bounded
hash table. `String`, `List`, `Dict`, and `Set` do not appear in the production object because
Mojo 1.0.0 links those heap-owning types to the compiler runtime. The final archive is checked for
unexpected `KGEN_CompilerRT_*` references.

`ContextTextRowsResult` is a `#[repr(C)]` nine-`Int64` result record. It reports ABI version,
input counts, rows written, unique-key count, signal-line count, and required row/key/hash
capacities. Statuses are `0=success`, `1=bounds or capacity`, `2=invalid pointer/text/counter`,
`3=key-table exhausted`, and `4=ABI mismatch`. Capacity failures fill the required-capacity
fields without reading line buffers; other failures may leave scratch/output buffers partially
written, and Rust discards them.

Rust owns every input, output, and scratch allocation. Inputs are immutable; mutable buffers must
be disjoint from inputs and from each other. All memory remains valid for one synchronous call.
Mojo neither retains pointers nor returns heap ownership, and the entry point is stateless and
reentrant. Raw pointer validity beyond null/length consistency remains an unsafe-caller
precondition; the public Rust wrapper accepts `&str` and constructs the records safely.

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
| Failure behavior | Mojo-feature builds fail when the compiler/archive is unavailable or a result is invalid; Rust-only builds use their separately compiled Rust path |

The 2026-08-21 additive exports are:

```text
prodex_runtime_optimistic_current_candidate_decision(
    route_kind, auth_failure_active, selection_backoff, circuit_open,
    health_score, performance_score, quota_compatible, alternative_quota_compatible,
    quota_band, quota_source_present, quota_source, inflight_count, inflight_soft_limit,
    prompt_cache_present, prompt_cache_owner_matches,
) -> Int64

prodex_smart_context_rehydrate_plan_batch(
    token_costs: *const UInt64, required: *const Int64, available: *const Int64,
    action_tags: *mut Int64, used_tokens: *mut UInt64,
    count: Int64, token_budget: UInt64, tier: Int64,
) -> Int64

prodex_quota_main_aggregate_batch(
    remaining_percent: *const Int64, remaining_present: *const Int64,
    reset_at: *const Int64, reset_present: *const Int64,
    profiles_with_data: *mut Int64, pool_remaining: *mut Int64,
    earliest_reset_at: *mut Int64, earliest_present: *mut Int64,
    count: Int64,
) -> Int64

prodex_runtime_tuning_defaults(
    parallelism: Int64,
    worker_count: *mut Int64, long_lived_worker_count: *mut Int64,
    probe_refresh_worker_count: *mut Int64, async_worker_count: *mut Int64,
    log_queue_capacity: *mut Int64, websocket_connect_worker_count: *mut Int64,
    websocket_dns_worker_count: *mut Int64,
) -> Int64

prodex_provider_constraints_evaluate_v2(
    input_i64: *const Int64, input_i64_count: Int64,
    input_u64: *const UInt64, input_u64_count: Int64,
    output_i64: *mut Int64, output_i64_count: Int64,
    output_u64: *mut UInt64, output_u64_count: Int64,
) -> Int64
```

Optimistic result tags are `Keep=0`, then `AuthFailure=1`, `SelectionBackoff=2`,
`RouteCircuit=3`, `Health=4`, `Performance=5`, `QuotaProbe=6`, `StalePersistedQuota=7`,
`QuotaThin=8`, `QuotaCritical=9`, `QuotaExhausted=10`, `QuotaUnknown=11`, `Inflight=12`,
`Incompatible=13`, and `PromptCache=14`. Prompt-cache strings and profile identity never cross
the boundary; Rust sends only presence and owner-match booleans.

Rehydration action tags are `Rehydrate=0`, `MissingArtifact=1`, `TokenBudgetExceeded=2`, and
`MinimalBudgetTier=3`; its maximum is 256 rows. Quota main aggregation accepts at most 1,024
rows and returns a saturating remaining sum plus an optional earliest reset. Runtime tuning
returns seven bounded integer defaults in one call; user overrides and environment parsing remain
Rust.

Provider constraints use decision tags `Compatible=0`, `EndpointUnsupported=1`,
`RequiredCapabilityMissing=2`, `CatalogEntryUnavailable=3`, `ContextWindowUnknown=4`,
`ContextWindowExceeded=5`, `OutputLimitUnknown=6`, `RequestedOutputExceedsModelLimit=7`,
`ReasoningReserveUnsupported=8`, `ReasoningReserveExcessive=9`, `MalformedRequestLimits=10`,
and `OutputLimitClamped=11`. Feature tags follow Rust enum order (`Tools=0` through
`Websocket=8`); output fields are `MaxOutputTokens=0`, `MaxCompletionTokens=1`, and
`MaxTokens=2`. Unknown-context tags are `Allow=0`, `SafeWindow=1`, `Reject=2`; oversized-output
tags are `Passthrough=0`, `Reject=1`, `ClampWithNotice=2`. Rust reconstructs warning-bit order.

Provider constraint ABI version `2` is independent of the shared routing ABI version. It requires
exact field counts of 17/7 input `Int64`/`UInt64` words and 12/5 output words. Input word zero and
output word zero carry the provider ABI version. Rust exposes typed enums rather than numeric tags;
unknown tags, invalid presence/value pairs, wrong counts, reserved decision output, and version
mismatch fail closed before a result reaches provider routing.

The shared routing ABI remains version `1`; provider constraints deliberately replace their
unversioned scalar ABI with the versioned buffer contract above. Rust owns every input/output
buffer and validates status, presence flags, tags, bounds, and conversions. Invalid Mojo results
return an error or fail an internal invariant; strict builds fail if current Mojo source cannot
compile.

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

## Rich/domain ABI v2 (2026-08-26)

Rich v2 is additive and does not overload text ABI v1. Its version entry point is
`prodex_mojo_rich_abi_version`, returning `2`; its layout probe is
`prodex_mojo_rich_abi_layout`. The current record family is:

| Record | Purpose |
| --- | --- |
| `RichStringView` | borrowed UTF-8 bytes (`ptr`, `len`); null plus zero length is allowed |
| `RichSlice` | offset/length into a caller-owned output byte arena |
| `RichContextRecord` / `RichContextResult` | diagnostic group objects and counts/capacity/error metadata |
| `RichRouteInput` / `RichRouteRecord` / `RichRouteResult` | provider/model/capability candidate graph and ordered decision |
| `RichPolicyInput` / `RichPolicyModel` / `RichPolicyResult` | route-alias parser input, normalized model records, and issue metadata |
| `RichPlanItem` / `RichPlanAction` / `RichPlanResult` | Smart Context item/action graph and budget result |

Rich operations are coarse-grained: one context text, route candidate set, policy alias, fallback
selector, or context-item set per call. Inputs are borrowed only for the synchronous call. Mojo
does not retain Rust pointers, and Rust never receives a Mojo object pointer. Object relationships
cross as indices and `RichSlice` offsets; generated strings are copied into a Rust-allocated byte
arena and validated as UTF-8 before reconstruction.

Status values are `0=ok`, `1=invalid boundary/input`, `2=invalid UTF-8`, `3=capacity`, and
`4=ABI mismatch`. Semantic failures are returned in the operation result as structured domain,
kind, field, object-index, byte-offset, and byte-length data and are mapped to `MojoIssue` by the
safe Rust wrapper. Capacity is normal control flow; the result reports required record, scratch,
and output sizes where the operation can determine them. Rust validates every status, version,
count, offset, length, UTF-8 slice, object index, type/reason tag, uniqueness constraint, and
ordering relation. Invalid output is a hard internal error on Mojo-enabled builds.

The layout probe covers all v2 record sizes and alignments. Current x86_64 evidence is 16-byte
string views/slices, 64-byte context records, 160-byte context results, 128-byte route inputs,
160-byte route records, 80-byte route results, 64-byte policy inputs, 32-byte policy models,
80-byte policy results, 32-byte plan items, 48-byte plan actions, and 72-byte plan results.
The ABI is reentrant and has no process-global mutable state; callers must provide disjoint buffers
for concurrent calls.

Production v2 entry points are `prodex_mojo_rich_context_analyze_v2`,
`prodex_mojo_rich_route_plan_v2`, `prodex_mojo_rich_policy_alias_v2`,
`prodex_mojo_rich_model_fallback_v2`, and `prodex_mojo_rich_context_plan_v2`. The feature-off Rust
implementation is a separate supported target and differential oracle, never a runtime fallback
after a Mojo-enabled call fails.
