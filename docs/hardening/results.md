# Hardening Results

This report separates controlled measurements from unavailable evidence. It
does not claim live-model quality, allocation, queue-wait, or lock-wait
improvements.

## Provenance

- Baseline: `c635d485cf750637512bfacb4e1cefc854ed1bef`.
- Final build/startup sample: current commit `375c257`.
- Earlier paired current build sample: commit `898fd3c`.
- Smart Context replay/performance sample: current commit `de60d09`.
- Host: Linux 7.0.0-28-generic, AMD Ryzen 5 PRO 4650G, 6 cores / 12
  threads, 30 GiB RAM.
- Toolchain: Rust 1.97.0, Cargo 1.97.0, Node 24.18.0, npm 12.0.1.
- CPU governor: `performance`; boost enabled.

Clean release builds used empty target trees with `/usr/bin/time -v`. Startup
used 100 successful `prodex --version` child processes per binary. RSS used 50
GNU `time` samples per binary. The complete retained current samples and all
recorded build values are in the [raw measurement report](raw-measurements.json).

## Structure

| Metric | Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| Workspace packages | 61 | 61 | 0 |
| Rust files | 2,020 | 1,991 | -29 |
| Rust physical lines | 555,876 | 553,427 | -2,449 (-0.44%) |
| Crate `src/` physical lines | 446,705 | 439,014 | -7,691 (-1.72%) |
| `prodex-app` direct normal dependencies | 90 | 89 | -1 |
| `prodex-app/src` Rust files | 716 | 697 | -19 |
| `prodex-app/src` physical lines | 203,074 | 198,179 | -4,895 (-2.41%) |
| `prodex-app/tests` physical lines | 71,682 | 68,579 | -3,103 (-4.33%) |
| Size-guard allowlist hits | 55 | 54 | -1 |

Every remaining `prodex-app` dependency has an ownership reason in
[the dependency inventory](prodex-app-dependencies.md).

## Build, Binary, And Startup

| Metric | Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| Clean release build wall time | 233.73 s | 184.72 s | -20.97% |
| Clean release build user time | 1,646.84 s | 1,520.79 s | -7.65% |
| Clean release build system time | 97.16 s | 88.13 s | -9.29% |
| Clean release build max RSS | 2,928,264 KiB | 2,856,224 KiB | -2.46% |
| Release binary | 54,153,624 bytes | 45,992,704 bytes | -8,160,920 (-15.07%) |
| ELF text | 40,328,863 bytes | 44,997,479 bytes | +11.58% |
| First startup | 5.895 ms | 4.343 ms | -26.33% |
| Warm startup p50 | 3.936 ms | 3.587 ms | -8.87% |
| Warm startup p95 | 6.128 ms | 4.161 ms | -32.09% |
| Warm startup p99 | 6.607 ms | 4.332 ms | -34.43% |
| Startup RSS p50 | 7,480 KiB | 7,864 KiB | +5.13% |
| Startup RSS p95 | 7,676 KiB | 8,008 KiB | +4.33% |
| Startup RSS p99 | 7,728 KiB | 8,032 KiB | +3.93% |

The binary reduction combines symbol stripping, bounded tokenizer linkage, and
asset removal. The isolated Caveman removal comparison was 54,142,968 bytes
before and 54,079,328 bytes after, a direct 63,640-byte reduction; the embedded
developer instruction was present before and absent after.

The earlier sequential paired current build was 235.86 seconds, only 0.91%
above baseline. The final empty-target sample was materially faster, showing
large machine/cache variance; no build-speed improvement is claimed. Final
startup latency did not regress, but the startup RSS median remained 384 KiB
(5.13%) above baseline. ELF writable data and the typed CLI/runtime surface are
plausible contributors, but the measurement does not isolate pages. This is a
reported regression and follow-up item, not a speed claim.

## Smart Context Correctness And Tokens

The generated [raw replay report](../generated/smart-context-replay-report.json)
and [summary](../generated/smart-context-replay-report.md) execute the production
engine against an inputs-only corpus.

| Metric | Baseline | Current |
| --- | --- | ---: |
| Valid deterministic scenarios | unavailable; old fixture contained self-asserted outputs | 18 |
| Exact input tokens | unavailable | 59,713 |
| Optimized input tokens | unavailable | 41,277 |
| Net tokenizer-counted tokens | unavailable | 18,436 |
| Deterministic failures | unavailable | 0 |

These are corpus-only tokenizer counts. They are not observed provider usage or
live-model task-quality evidence.

## Smart Context Performance

The generated [raw performance report](../generated/smart-context-performance-report.json)
and [summary](../generated/smart-context-performance-report.md) retain all 50
per-case samples. Selected results:

| Case | p50 | p95 | p99 |
| --- | ---: | ---: | ---: |
| Disabled, 240 KiB | 8.06 ns | 8.83 ns | 9.22 ns |
| Exact, 240 KiB | 16.61 ns | 17.72 ns | 18.13 ns |
| Canary-out, 240 KiB | 5.172 us | 6.256 us | 6.700 us |
| Rejected/no-op, 240 KiB | 196.226 us | 205.736 us | 210.578 us |
| Active duplicate rewrite, 64 KiB | 9.885 ms | 10.575 ms | 10.948 ms |
| Active duplicate rewrite, 240 KiB | 37.689 ms | 39.450 ms | 39.651 ms |
| Sampled shadow, 240 KiB | 36.113 ms | 37.784 ms | 39.239 ms |
| Legacy artifact rehydration | 75.685 us | 80.976 us | 83.831 us |

Release rewriting is bounded to 256 KiB for HTTP, 96 KiB for WebSocket, and a
100 ms pre-commit deadline. Debug builds use a 5 s deadline because unoptimized
tokenizer execution is materially slower. Rejected/no-op p95 remains below 1
ms across the measured sizes.

The old active benchmark used one output and unsafe heuristic artifact
semantics; the current benchmark uses a real duplicate, full serialization,
tokenizer counts, expansion, and lossless validation. Its same-host old median
was 1.724 ms and p90 was 1.791 ms; the new production-shaped case is slower but
not semantically comparable. The requested 5% matched active-rewrite claim is
therefore unproven and is not made.

Allocation/request, Smart Context CPU/request, queue wait, per-scope lock wait,
and Smart Context-specific RSS were not captured. The process-global state lock
was deleted and statically guarded, but that is architectural evidence, not a
latency distribution.

## Runtime Load And Stress

The same 32-request mock-upstream baseline scenario completed without request
errors or admission pressure before and after.

| Metric | Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| TTFT p50 | 52.19 ms | 52.71 ms | +1.00% |
| TTFT p95 | 71.33 ms | 66.77 ms | -6.39% |
| TTFT p99 | 92.01 ms | 92.88 ms | +0.95% |
| Completion p50 | 111.99 ms | 113.02 ms | +0.92% |
| Completion p95 | 130.64 ms | 127.20 ms | -2.63% |
| Completion p99 | 150.69 ms | 152.98 ms | +1.52% |

`npm run ci:runtime-stress` passed its 458-test primary serialized shard and
its repeated continuation cases. The checked hot-path benchmark passed all
eight thresholds; the Smart Context large-tool-output case measured a 6.998 ms
median and 7.121 ms p90 against an 8 ms limit.

## Final Validation

The required Cargo metadata, formatting, Clippy, workspace test, focused Smart
Context, npm, documentation, boundary, hot-path, load, stress, supply-chain,
secret, dependency-duplicate, size, audit, deny, release-build, and benchmark
commands passed. The workspace test result was 3,099 tests across 192 suites.
Focused Smart Context results were 116 runtime-proxy tests and 100 app tests.
The obsolete `runtime_caveman` filter exited successfully with zero matching
tests because that module was deleted; replacement optional-tool and Claude
coverage is active.

The dated-nightly fuzz build passed for every target. The new bounded Smart
Context target completed 393,417 AddressSanitizer runs in 11 seconds without a
crash. Nineteen artifact migration, scope, ordering, eviction, and concurrent
merge tests passed separately.

Windows security/workspace jobs exist in `.github/workflows/ci.yml`, but this
Linux run did not execute them. Preflight skipped the optional live PostgreSQL
proof because `PRODEX_TEST_POSTGRES_URL` was absent. No live-model quality test
was run. These are explicit environmental/non-deterministic gaps, not passes.

## Reproduction Commands

```bash
/usr/bin/time -v cargo build --release --locked
npm run smart-context:replay
npm run docs:smart-context-evidence:check
PRODEX_RUNTIME_PROXY_BENCH_CHECK=1 cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths
cargo +nightly-2026-07-11 fuzz run smart_context_inputs --fuzz-dir fuzz -- -max_total_time=10 -max_len=65536
cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths -- \
  runtime_smart_context_ --noplot --warm-up-time 1 --measurement-time 2 --sample-size 50
npm run bench:smart-context-report
```

Environmental or repository failures remain failures until explicitly
resolved; unmeasured values are not inferred.
