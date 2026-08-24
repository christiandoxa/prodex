# Migration benchmarks

The current provider work is a bounded routing-plan batch and a capability-match batch.
The measurements below cover the public Rust boundary, including Rust normalization,
flat-buffer conversion, FFI, and result reconstruction. They are Criterion ranges from
short local runs, not release-performance claims.

| Check | Command / observation | Result |
| --- | --- | --- |
| Mojo compiler | `mojo --version` | `Mojo 1.0.0 (ed45d567)` |
| Mojo object build | `mojo build mojo/prodex_core/quota.mojo --emit object --optimization-level=3 -o ...` | Pass; exported symbol present |
| Rust linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-quota --features mojo` | 55 tests pass, including strict activation and C-ABI smoke |
| Runtime-proxy linkage | `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-runtime-proxy --features mojo` | 409 library tests plus 1 activation test pass, including route-pressure, profile-scheduling, and byte-estimate parity |
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
| Quota parity | `cargo test --locked -q -p prodex-quota --features mojo -- --test-threads=1` | 55 passed |
| Runtime proxy parity | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo test --locked -q -p prodex-runtime-proxy --features mojo --lib -- --test-threads=1` | 409 passed; runtime candidate ordering and Smart Context pressure use the Mojo path |
| Runtime parity | `PRODEX_MOJO_REQUIRED=1 ... cargo test --locked -p prodex-mojo-core --features mojo-core --test runtime_parity -- --test-threads=1` | Runtime candidate and pressure parity fixtures pass |
| Profile scheduling parity | `PRODEX_MOJO_REQUIRED=1 ... cargo test --locked -p prodex-mojo-core --features mojo-core --test profile_schedule -- --test-threads=1` | Stable ordering and pressure-boundary fixtures pass |
| Mojo core strict suite | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo test --locked -q -p prodex-mojo-core --features mojo-core -- --test-threads=1` | Shared archive and self-test pass |
| Doctor module diagnostics | `PRODEX_MOJO_REQUIRED=1 ... cargo test --locked -q -p prodex-app --features mojo-core --lib app_commands::doctor -- --test-threads=1` | 10 passed; aggregate contract plus independent runtime modules |
| Runtime quota parity | `cargo test --locked -q -p prodex-runtime-quota --features mojo -- --test-threads=1` | 17 passed |
| Provider routing and capability strict suite | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo test --locked -q -p prodex-provider-spi --features mojo -- --test-threads=1` | 2 + 15 + 16 tests passed across 4 suites; provider score oracle, governed routing, and capability negotiation paths exercised |
| Root `mojo-routing` feature wiring | `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 cargo check --locked -q --features mojo-routing --bin prodex` | Pass |
| Optimized direct release | `cargo build --release --locked --target x86_64-unknown-linux-gnu --features mojo-core --bin prodex` | Host-link evidence is separate from the release artifact and must not be used as the GLIBC promotion result |
| Target archive handoff | `PRODEX_MOJO_ARCHIVE=target/mojo-release/x86_64-unknown-linux-gnu/libprodex_mojo_core.a cross build --release ...` | Pass locally; final ELF stays within the `GLIBC_2.23` release ceiling, has no dynamic Mojo dependency/RPATH, and executes the Mojo self-test without `mojo` |
| Final binary dependencies | `ldd` / `readelf --dynamic` on optimized Mojo binary | Only libc/libm/libgcc/loader; no Mojo/Modular dependency or RPATH/RUNPATH observed |
| Optimized binary size | `stat` on current stripped host/cross Linux artifacts | 49,681,016 bytes host link; 50,202,496 bytes GLIBC-compatible cross link |
| Installer fixture | Local file-backed `install.sh` with no Mojo on `PATH` | Pass; manifest/checksum selection and Mojo self-test verified |

## 2026-08-21 complete-boundary measurements

The provider constraint benchmark includes request JSON parsing, Rust requirement normalization,
catalog resolution, Mojo ABI conversion/evaluation, and Rust public-model reconstruction. It was
run with the same `provider_constraints_complete_boundary` Criterion target in strict Mojo and
Rust-only builds:

| Boundary | Command mode | Result |
| --- | --- | --- |
| Provider request constraints | Rust-only build without the Mojo feature | `[2.4896 µs 2.5925 µs 2.7138 µs]` |
| Active Smart Context rehydration | `PRODEX_MOJO_REQUIRED=1`, `runtime_smart_context_rehydrate` | `[53.226 µs 54.058 µs 54.916 µs]` |

The Mojo boundary is within local noise of the Rust-only baseline for this small single-request
workload; no speedup is claimed. The complete-boundary benchmark is retained so future batching
can be measured honestly before changing ownership again. The new rehydration boundary has
2,000-case differential coverage and is also exercised by the existing
`runtime_smart_context_rehydrate` Criterion path. This boundary includes the active app-side
artifact normalization/store work, plan construction, and Rust-side execution accounting.

The GLIBC result is intentionally recorded as a limitation of a directly linked host binary.
Release CI compiles the Mojo target archive first and links the final Linux binary through the
target cross toolchain; only that final artifact is eligible for publication.

## 2026-08-24 context text boundary

`context_text_boundary` measures the public lost-range operation, including Rust terminal/line
normalization and classification, record conversion, one rich text FFI call, Mojo UTF-8
validation/grouping, the existing Mojo range planner, and Rust result reconstruction. Both modes
used the same local machine, 200 ms warm-up, 500 ms measurement, and 20 samples. Counts are lines
per `before` and `after` input.

| Lines per side | Rust-only midpoint | Mojo midpoint | Delta |
| ---: | ---: | ---: | ---: |
| 0 | 53.451 ns | 68.978 ns | +29.1% (early-return noise; no text call) |
| 1 | 21.410 µs | 20.220 µs | -5.6% |
| 16 | 493.10 µs | 462.08 µs | -6.3% |
| 64 | 1.5197 ms | 1.4731 ms | -3.1% |
| 256 | 5.6936 ms | 5.5198 ms | -3.1% |
| 1,024 | 23.098 ms | 21.401 ms | -7.3% |

At 64 lines per side, isolated shapes measured: ASCII `132.81/140.71 µs`, Unicode
`167.88/174.56 µs`, duplicates `142.30/140.61 µs`, long 8 KiB lines `9.6640/10.467 ms`, and
adversarial prefixes `2.0347/2.0087 ms` for Rust/Mojo. The long-line case is 8.3% slower and is
the current worst observed tradeoff; mixed production-shaped batches improve by 3.1% through
7.3% from 64 through 1,024 lines.

The direct 64/63-line `prepare_signal_rows` wrapper, including Rust view/counter allocation, FFI,
Mojo work, and Rust validation/reconstruction, measured `[10.360 µs 10.659 µs 11.007 µs]`.
Rust normalization/classification of the two mixed 64-line inputs measured
`[845.68 µs 859.75 µs 873.02 µs]`. The complete Mojo operation measured 1.4731 ms, so the
direct rich boundary is about 0.7% of end-to-end time on this corpus. Mojo-internal time cannot be
separated from the ABI call without adding production instrumentation and is not claimed.

Commands:

```bash
cargo bench --locked -q -p prodex-context --bench context_text_boundary -- \
  --noplot --warm-up-time 0.2 --measurement-time 0.5 --sample-size 20
PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.0.0 \
  cargo bench --locked -q -p prodex-context --features mojo \
  --bench context_text_boundary -- \
  --noplot --warm-up-time 0.2 --measurement-time 0.5 --sample-size 20
```

These are local architecture measurements, not cross-machine release thresholds.
