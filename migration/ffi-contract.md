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

`crates/prodex-quota/build.rs` and `crates/prodex-runtime-proxy/build.rs`:

1. runs only when feature `mojo` is enabled;
2. invokes `mojo build --emit object --optimization-level=3`;
3. archives the generated object as `libprodex_quota_mojo.a`;
4. forwards that static library to final Cargo link targets;
5. accepts `PRODEX_MOJO` and `AR` only as local tool path overrides;
6. emits `prodex_mojo_active` only after the current source object is archived;
7. treats `PRODEX_MOJO_REQUIRED=1` as strict mode: a disabled feature, missing
   compiler/archiver, or failed build is a hard error;
8. never downloads tools or invokes network access.

The object path is a Cargo build artifact. It must not be committed or copied into a
release by this slice. Cross-compilation and non-Linux targets need an explicit follow-up
probe before enabling the feature there.

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
