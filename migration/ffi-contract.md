# Rust-Mojo FFI contract

## Current boundary

Rust calls four exported Mojo functions when the `prodex-quota/mojo` Cargo feature is
enabled:

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

The symbol uses the platform C ABI. `has_value` is `0` for Rust `None` and `1` for
Rust `Some(_)`; the `used_percent` value is ignored when `has_value == 0`.

The policy entry points use explicit integer tags:

| Concept | Codes |
| --- | --- |
| Window status | `Ready=0`, `Thin=1`, `Critical=2`, `Exhausted=3`, `Unknown=4` |
| Pressure band | `Healthy=0`, `Thin=1`, `Critical=2`, `Exhausted=3`, `Unknown=4` |
| Boolean result | `0=false`, `1=true` |

## Ownership and lifetime

| Concern | Contract |
| --- | --- |
| Allocation | None. Both arguments and return value are scalar registers. |
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
prodex_runtime_quota_profile_score_batch(..., count: Int64) -> Int64
prodex_smart_context_estimate_tokens_from_body_bytes(body_bytes: UInt64) -> UInt64
prodex_routing_score_batch(..., count: Int64, weights...) -> Int64
```

The batch calls use parallel flat `Int64` arrays and caller-owned output arrays. They accept at
most 64 records. A zero-length batch is valid and does not dereference any pointer. Non-zero
batches require every pointer to reference at least `count` input or output elements; null
non-zero pointers are invalid and must never be passed. Rust keeps all vectors alive for the
synchronous call, and Mojo does not retain any pointer after return.

The routing batch writes seven normalized components, one weighted total, and one final score per
record. The runtime quota batch writes four `Int64` values per record. Status `0` means success;
non-zero statuses mean invalid bounded input and the Rust wrapper keeps the Rust oracle result.
The wrapper also validates integer conversions before exposing results to its callers.

| Concern | Batch contract |
| --- | --- |
| Allocation | Rust allocates all flat input/output vectors; Mojo allocates nothing across the ABI |
| Ownership | Rust owns and may mutate output buffers; Mojo only reads inputs and writes outputs during the call |
| Lifetime | Pointer lifetime is one synchronous call; no pointer escapes |
| Alignment | Rust slices provide native `i64` alignment; Mojo pointers are typed `Int64` pointers |
| Thread safety | Functions are stateless and reentrant; concurrent calls use disjoint caller-owned buffers |
| Nullability | Null is valid only for zero-count buffers; Rust uses zero-length slices and Mojo returns before dereference |
| Error propagation | Explicit integer status; no panic, exception, Rust enum, `Vec`, `String`, or `Result` crosses the boundary |
| Fallback | Build-time missing Mojo selects the documented Rust fallback; strict builds fail. Invalid runtime batch input is handled by the Rust oracle and is not a compiler fallback |

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

No generated object or archive is committed. The final Linux release is verified for static
Mojo dependencies, no build-path RPATH/RUNPATH, GLIBC policy, and execution without `mojo` on
`PATH`.
