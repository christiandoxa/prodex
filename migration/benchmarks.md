# Migration benchmarks

The current provider work is a bounded routing-plan batch and a capability-match batch.
The measurements below cover the public Rust boundary, including Rust normalization,
flat-buffer conversion, FFI, and result reconstruction. They are Criterion ranges from
short local runs, not release-performance claims.

| Check | Command / observation | Result |
| --- | --- | --- |
| Mojo compiler | `mojo --version` | `Mojo 1.0.0 (ed45d567)` |
| Mojo object build | `mojo build mojo/prodex_core/quota.mojo --emit object --optimization-level=3 -o ...` | Pass; exported symbol present |
| Rust linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-quota --features mojo` | 53 tests pass, including strict activation and C-ABI smoke |
| Runtime-proxy linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-runtime-proxy --features mojo` | 406 tests plus 1 activation test pass, including route-pressure, profile-score, and byte-estimate parity |
| Rust fallback | `PATH=/home/doxa/.cargo/bin:/usr/bin:/bin cargo test --locked -q -p prodex-quota --features mojo` | 50 tests pass without a Mojo compiler |
| Linked binary | `PRODEX_MOJO_REQUIRED=1 cargo build --locked --features mojo-core --bin prodex` | `prodex --version` runs; no Mojo/Modular shared dependency in `ldd` |
| Complete routing boundary | `cargo bench ... governance_routing_...` with 0, 1, and 64 candidates | Rust-only: representative `[707.19 ns 708.20 ns 709.51 ns]`; max `[6.7758 µs 6.7858 µs 6.7961 µs]`; circuit fallback `[715.63 ns 722.66 ns 730.67 ns]` |
| Complete routing boundary with Mojo | `PRODEX_MOJO_REQUIRED=1 ... cargo bench --features mojo-routing ... governance_routing_...` | Representative `[1.4197 µs 1.4279 µs 1.4358 µs]`; max `[15.083 µs 15.105 µs 15.135 µs]`; circuit fallback `[1.4133 µs 1.4274 µs 1.4444 µs]` |
| Complete capability boundary | `cargo bench ... governance_capability_match_max_candidates` with 64 candidates | Rust-only `[5.7791 µs 5.8118 µs 5.8450 µs]` |
| Complete capability boundary with Mojo | `PRODEX_MOJO_REQUIRED=1 ... cargo bench --features mojo-routing ... governance_capability_match_max_candidates` | `[7.0472 µs 7.0998 µs 7.1668 µs]` |

## Measurement rule

Benchmark the complete coarse-grained candidate in Rust and Mojo, including Rust
normalization, flat-buffer conversion, FFI overhead, and Rust plan reconstruction. Record
throughput, latency, allocations where measurable, and startup cost. Do not promote a Mojo
implementation because its inner loop is faster if the complete boundary is slower or less
maintainable.

The measured Mojo calls are slower on these local workloads because the complete boundary
includes conversion and archive-call overhead. The kernels remain promoted where exact parity,
coarse-grained ownership, and the existing compiled-in architecture justify that cost; no
performance improvement is claimed.

## Current migration evidence

| Check | Command / observation | Result |
| --- | --- | --- |
| Shared strict build | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo build --locked --features mojo-core --bin prodex` | Pass when run; binary must report compiled-in Mojo, `compiler_required=false`, and a passing self-test |
| Quota parity | `cargo test --locked -q -p prodex-quota --features mojo -- --test-threads=1` | 53 passed |
| Runtime proxy parity | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo test --locked -q -p prodex-runtime-proxy --features mojo --lib -- --test-threads=1` | 406 passed; runtime candidate ordering and Smart Context pressure use the Mojo path |
| Runtime generated parity | `PRODEX_MOJO_REQUIRED=1 ... cargo test --locked -p prodex-mojo-core --features mojo-core runtime::parity_tests -- --nocapture --test-threads=1` | 2 tests passed; 300 fixed-seed runtime candidate cases plus 300 pressure cases |
| Runtime proxy Rust fallback | `PRODEX_MOJO_REQUIRED=0 PATH=/home/doxa/.cargo/bin:/usr/bin:/bin cargo test --locked -q -p prodex-runtime-proxy --features mojo --lib -- --test-threads=1` | 406 passed without a Mojo compiler |
| Mojo core strict suite | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo test --locked -q -p prodex-mojo-core --features mojo-core -- --test-threads=1` | 3 passed; shared archive and self-test pass |
| Mojo core fallback suite | `PRODEX_MOJO_REQUIRED=0 PATH=/home/doxa/.cargo/bin:/usr/bin:/bin cargo test --locked -q -p prodex-mojo-core --features mojo-core -- --test-threads=1` | 1 passed without a Mojo compiler |
| Doctor module diagnostics | `PRODEX_MOJO_REQUIRED=1 ... cargo test --locked -q -p prodex-app --features mojo-core --lib app_commands::doctor -- --test-threads=1` | 10 passed; aggregate contract plus independent runtime modules |
| Runtime quota parity | `cargo test --locked -q -p prodex-runtime-quota --features mojo -- --test-threads=1` | 17 passed |
| Provider routing and capability strict suite | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo test --locked -q -p prodex-provider-spi --features mojo -- --test-threads=1` | 2 + 15 + 16 tests passed across 4 suites; provider score oracle, governed routing, and capability negotiation paths exercised |
| Provider routing and capability Rust fallback | `PATH=/home/doxa/.cargo/bin:/usr/bin:/bin PRODEX_MOJO_REQUIRED=0 cargo test --locked -q -p prodex-provider-spi --features mojo -- --test-threads=1` | 2 + 15 + 16 tests passed across 4 suites without a Mojo compiler on `PATH` |
| Root `mojo-routing` feature wiring | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo check --locked -q --features mojo-routing --bin prodex` | Pass |
| Optimized direct release | `cargo build --release --locked --target x86_64-unknown-linux-gnu --features mojo-core --bin prodex` | Host-link evidence is separate from the release artifact and must not be used as the GLIBC promotion result |
| Target archive handoff | `PRODEX_MOJO_ARCHIVE=target/mojo-release/x86_64-unknown-linux-gnu/libprodex_mojo_core.a cross build --release ...` | Pass locally; final ELF requires at most `GLIBC_2.18`, has no dynamic Mojo dependency/RPATH, and executes the Mojo self-test without `mojo` |
| Final binary dependencies | `ldd` / `readelf --dynamic` on optimized Mojo binary | Only libc/libm/libgcc/loader; no Mojo/Modular dependency or RPATH/RUNPATH observed |
| Optimized binary size | `stat` on current stripped host/cross Linux artifacts | 49,681,016 bytes host link; 50,202,496 bytes GLIBC-compatible cross link |
| Installer fixture | Local file-backed `install.sh` with no Mojo on `PATH` | Pass; manifest/checksum selection and Mojo self-test verified |

## 2026-08-21 complete-boundary measurements

The provider constraint benchmark includes request JSON parsing, Rust requirement normalization,
catalog resolution, Mojo ABI conversion/evaluation, and Rust public-model reconstruction. It was
run with the same `provider_constraints_complete_boundary` Criterion target in the strict and
fallback builds:

| Boundary | Command mode | Result |
| --- | --- | --- |
| Provider request constraints | `PRODEX_MOJO_REQUIRED=1`, real Mojo 1.0.0 | `[2.6560 µs 2.8658 µs 3.1268 µs]` |
| Provider request constraints | `PRODEX_MOJO_REQUIRED=0`, Mojo absent from `PATH` | `[2.4896 µs 2.5925 µs 2.7138 µs]` |
| Active Smart Context rehydration | `PRODEX_MOJO_REQUIRED=1`, `runtime_smart_context_rehydrate` | `[53.226 µs 54.058 µs 54.916 µs]` |

The Mojo boundary is within local noise of the Rust fallback for this small single-request
workload; no speedup is claimed. The complete-boundary benchmark is retained so future batching
can be measured honestly before changing ownership again. The new rehydration boundary has
2,000-case differential coverage and is also exercised by the existing
`runtime_smart_context_rehydrate` Criterion path. This boundary includes the active app-side
artifact normalization/store work, plan construction, and Rust-side execution accounting.

The GLIBC result is intentionally recorded as a limitation of a directly linked host binary.
Release CI compiles the Mojo target archive first and links the final Linux binary through the
target cross toolchain; only that final artifact is eligible for publication.
