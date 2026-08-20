# Migration benchmarks

The current provider work is a bounded routing-plan batch and a capability-match batch.
No performance benchmark has been run for those complete paths; an isolated Mojo inner-loop
measurement would not include Rust normalization, FFI conversion, or plan reconstruction.

| Check | Command / observation | Result |
| --- | --- | --- |
| Mojo compiler | `mojo --version` | `Mojo 1.0.0 (ed45d567)` |
| Mojo object build | `mojo build mojo/prodex_core/quota.mojo --emit object --optimization-level=3 -o ...` | Pass; exported symbol present |
| Rust linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-quota --features mojo` | 53 tests pass, including strict activation and C-ABI smoke |
| Runtime-proxy linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-runtime-proxy --features mojo` | 406 tests plus 1 activation test pass, including route-pressure, profile-score, and byte-estimate parity |
| Rust fallback | `PATH=/home/doxa/.cargo/bin:/usr/bin:/bin cargo test --locked -q -p prodex-quota --features mojo` | 50 tests pass without a Mojo compiler |
| Linked binary | `PRODEX_MOJO_REQUIRED=1 cargo build --locked --features mojo-core --bin prodex` | `prodex --version` runs; no Mojo/Modular shared dependency in `ldd` |
| Performance | Not measured for complete routing/capability paths or domain FFI calls | Defer until the Rust and Mojo workloads include conversion, FFI, and reconstruction |

## Measurement rule

Benchmark the complete coarse-grained candidate in Rust and Mojo, including Rust
normalization, flat-buffer conversion, FFI overhead, and Rust plan reconstruction. Record
throughput, latency, allocations where measurable, and startup cost. Do not promote a Mojo
implementation because its inner loop is faster if the complete boundary is slower or less
maintainable.

## Current migration evidence

| Check | Command / observation | Result |
| --- | --- | --- |
| Shared strict build | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo build --locked --features mojo-core --bin prodex` | Pass when run; binary must report compiled-in Mojo, `compiler_required=false`, and a passing self-test |
| Quota parity | `cargo test --locked -q -p prodex-quota --features mojo -- --test-threads=1` | 53 passed |
| Runtime proxy parity | `cargo test --locked -q -p prodex-runtime-proxy --features mojo -- --test-threads=1` | 406 passed plus 1 activation test |
| Runtime quota parity | `cargo test --locked -q -p prodex-runtime-quota --features mojo -- --test-threads=1` | 17 passed |
| Provider routing and capability strict suite | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo test --locked -q -p prodex-provider-spi --features mojo -- --test-threads=1` | 2 + 15 + 16 tests passed across 4 suites; provider score oracle, governed routing, and capability negotiation paths exercised |
| Provider routing and capability Rust fallback | `PATH=/home/doxa/.cargo/bin:/usr/bin:/bin PRODEX_MOJO_REQUIRED=0 cargo test --locked -q -p prodex-provider-spi --features mojo -- --test-threads=1` | 2 + 15 + 16 tests passed across 4 suites without a Mojo compiler on `PATH` |
| Root `mojo-routing` feature wiring | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo check --locked -q --features mojo-routing --bin prodex` | Pass |
| Optimized direct release | `cargo build --release --locked --target x86_64-unknown-linux-gnu --features mojo-core --bin prodex` | Host-link evidence is separate from the release artifact and must not be used as the GLIBC promotion result |
| Target archive handoff | `PRODEX_MOJO_ARCHIVE=target/mojo-release/x86_64-unknown-linux-gnu/libprodex_mojo_core.a cross build --release ...` | Pass locally; final ELF requires at most `GLIBC_2.18`, has no dynamic Mojo dependency/RPATH, and executes the Mojo self-test without `mojo` |
| Final binary dependencies | `ldd` / `readelf --dynamic` on optimized Mojo binary | Only libc/libm/libgcc/loader; no Mojo/Modular dependency or RPATH/RUNPATH observed |
| Optimized binary size | `stat` on current stripped host/cross Linux artifacts | 49,681,016 bytes host link; 50,202,496 bytes GLIBC-compatible cross link |
| Installer fixture | Local file-backed `install.sh` with no Mojo on `PATH` | Pass; manifest/checksum selection and Mojo self-test verified |

The GLIBC result is intentionally recorded as a limitation of a directly linked host binary.
Release CI compiles the Mojo target archive first and links the final Linux binary through the
target cross toolchain; only that final artifact is eligible for publication.
