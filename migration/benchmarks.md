# Migration benchmarks

The first slice is one scalar quota conversion. An isolated per-value benchmark would
mostly measure FFI call overhead, so it is not evidence for moving a larger algorithm.

| Check | Command / observation | Result |
| --- | --- | --- |
| Mojo compiler | `mojo --version` | `Mojo 1.0.0 (ed45d567)` |
| Mojo object build | `mojo build mojo/prodex_core/quota.mojo --emit object --optimization-level=3 -o ...` | Pass; exported symbol present |
| Rust linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-quota --features mojo` | 53 tests pass, including strict activation and C-ABI smoke |
| Runtime-proxy linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-runtime-proxy --features mojo` | 405 tests pass, including route-pressure parity |
| Rust fallback | `PATH=/home/doxa/.cargo/bin:/usr/bin:/bin cargo test --locked -q -p prodex-quota --features mojo` | 50 tests pass without a Mojo compiler |
| Linked binary | `PRODEX_MOJO_REQUIRED=1 cargo build --locked --features mojo-quota --bin prodex` | `prodex --version` runs; no Mojo/Modular shared dependency in `ldd` |
| Performance | Not measured | Defer until a batch candidate exists |

## Measurement rule

Benchmark the complete coarse-grained candidate in Rust and Mojo, including conversion
and FFI overhead. Record throughput, latency, allocations where measurable, and startup
cost. Do not promote a Mojo implementation because its inner loop is faster if the
boundary makes the complete path slower or less maintainable.
