# Migration benchmarks

The first slice is one scalar quota conversion. An isolated per-value benchmark would
mostly measure FFI call overhead, so it is not evidence for moving a larger algorithm.

| Check | Command / observation | Result |
| --- | --- | --- |
| Mojo compiler | `mojo --version` | `Mojo 1.0.0 (ed45d567)` |
| Mojo object build | `mojo build mojo/prodex_core/quota.mojo --emit object --optimization-level=3 -o ...` | Pass; exported symbol present |
| Rust linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-quota --features mojo` | 53 tests pass, including strict activation and C-ABI smoke |
| Runtime-proxy linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-runtime-proxy --features mojo` | 406 tests plus 1 activation test pass, including route-pressure, profile-score, and byte-estimate parity |
| Rust fallback | `PATH=/home/doxa/.cargo/bin:/usr/bin:/bin cargo test --locked -q -p prodex-quota --features mojo` | 50 tests pass without a Mojo compiler |
| Linked binary | `PRODEX_MOJO_REQUIRED=1 cargo build --locked --features mojo-core --bin prodex` | `prodex --version` runs; no Mojo/Modular shared dependency in `ldd` |
| Performance | Not measured | Defer until a batch candidate exists |

## Measurement rule

Benchmark the complete coarse-grained candidate in Rust and Mojo, including conversion
and FFI overhead. Record throughput, latency, allocations where measurable, and startup
cost. Do not promote a Mojo implementation because its inner loop is faster if the
boundary makes the complete path slower or less maintainable.

## Current migration evidence

| Check | Command / observation | Result |
| --- | --- | --- |
| Shared strict build | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo build --locked --features mojo-core --bin prodex` | Pass; binary reports compiled-in Mojo and self-test value `58` |
| Quota parity | `cargo test --locked -q -p prodex-quota --features mojo -- --test-threads=1` | 53 passed |
| Runtime proxy parity | `cargo test --locked -q -p prodex-runtime-proxy --features mojo -- --test-threads=1` | 406 passed plus 1 activation test |
| Runtime quota parity | `cargo test --locked -q -p prodex-runtime-quota --features mojo -- --test-threads=1` | 17 passed |
| Provider score parity | `cargo test --locked -q -p prodex-provider-spi --features mojo -- --test-threads=1` | 2 + 15 + 16 tests passed |
| Optimized direct release | `cargo build --release --locked --target x86_64-unknown-linux-gnu --features mojo-core --bin prodex` | Links and runs Mojo, but local host requires up to `GLIBC_2.39`; this is not the release artifact |
| Target archive handoff | `PRODEX_MOJO_ARCHIVE=target/mojo-release/x86_64-unknown-linux-gnu/libprodex_mojo_core.a cross build --release ...` | Pass locally; final ELF requires at most `GLIBC_2.18`, has no dynamic Mojo dependency/RPATH, and executes the Mojo self-test without `mojo` |
| Final binary dependencies | `ldd` / `readelf --dynamic` on optimized Mojo binary | Only libc/libm/libgcc/loader; no Mojo/Modular dependency or RPATH/RUNPATH observed |
| Optimized binary size | `stat` on current stripped host/cross Linux artifacts | 49,681,016 bytes host link; 50,202,496 bytes GLIBC-compatible cross link |
| Installer fixture | Local file-backed `install.sh` with no Mojo on `PATH` | Pass; manifest/checksum selection and Mojo self-test verified |

The GLIBC result is intentionally recorded as a limitation of a directly linked host binary.
Release CI compiles the Mojo target archive first and links the final Linux binary through the
target cross toolchain; only that final artifact is eligible for publication.
