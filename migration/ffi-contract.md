# Rust-Mojo FFI contract

## Current boundary

Rust calls one exported Mojo function when the `prodex-quota/mojo` Cargo feature is
enabled:

```text
prodex_quota_remaining_percent(used_percent: Int64, has_value: Int64) -> Int64
```

The symbol uses the platform C ABI. `has_value` is `0` for Rust `None` and `1` for
Rust `Some(_)`; the `used_percent` value is ignored when `has_value == 0`.

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

Rust retains the original implementation as the default path and as the differential
oracle. Mojo is opt-in because Cargo builds must remain usable on machines without the
Mojo compiler. If the opt-in feature finds no compiler or archiver on `PATH`, Cargo emits
an explicit warning and uses the Rust implementation; an explicitly configured but
failing tool or a Mojo compile error fails the build.

## Build contract

`crates/prodex-quota/build.rs`:

1. runs only when feature `mojo` is enabled;
2. invokes `mojo build --emit object --optimization-level=3`;
3. archives the generated object as `libprodex_quota_mojo.a`;
4. forwards that static library to final Cargo link targets;
5. accepts `PRODEX_MOJO` and `AR` only as local tool path overrides;
6. never downloads tools or invokes network access.

The object path is a Cargo build artifact. It must not be committed or copied into a
release by this slice. Cross-compilation and non-Linux targets need an explicit follow-up
probe before enabling the feature there.
