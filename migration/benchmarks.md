# Migration benchmarks

The first slice is one scalar quota conversion. An isolated per-value benchmark would
mostly measure FFI call overhead, so it is not evidence for moving a larger algorithm.

| Check | Command / observation | Result |
| --- | --- | --- |
| Mojo compiler | `mojo --version` | `Mojo 1.0.0 (ed45d567)` |
| Mojo object build | `mojo build mojo/prodex_core/quota.mojo --emit object --optimization-level=3 -o ...` | Pass; exported symbol present |
| Rust linkage | `cargo test --locked -q -p prodex-quota --features mojo` | Parity test pass |
| Rust fallback | `cargo test --locked -q -p prodex-quota` | Rust-only path pass |
| Performance | Not measured | Defer until a batch candidate exists |

## Measurement rule

Benchmark the complete coarse-grained candidate in Rust and Mojo, including conversion
and FFI overhead. Record throughput, latency, allocations where measurable, and startup
cost. Do not promote a Mojo implementation because its inner loop is faster if the
boundary makes the complete path slower or less maintainable.
