# Hardening Results

This report separates controlled measurements from unavailable evidence. It
does not claim live-model quality, allocation, queue-wait, or lock-wait
improvements.

## Provenance

- Baseline: `c635d485cf750637512bfacb4e1cefc854ed1bef`.
- Controlled build/startup sample: current commit `898fd3c`.
- Smart Context replay/performance sample: current commit `de60d09`.
- Host: Linux 7.0.0-28-generic, AMD Ryzen 5 PRO 4650G, 6 cores / 12
  threads, 30 GiB RAM.
- Toolchain: Rust 1.97.0, Cargo 1.97.0, Node 24.18.0, npm 12.0.1.
- CPU governor: `performance`; boost enabled.

The two clean release builds ran sequentially from empty target trees with
`/usr/bin/time -v`. Startup used 100 successful `prodex --version` child
processes per binary. RSS used 50 interleaved GNU `time` samples per binary.

## Structure

| Metric | Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| Workspace packages | 61 | 61 | 0 |
| Rust files | 2,020 | 1,990 | -30 |
| Rust physical lines | 555,876 | 553,428 | -2,448 (-0.44%) |
| Crate `src/` physical lines | 446,705 | 439,065 | -7,640 (-1.71%) |
| `prodex-app` direct normal dependencies | 90 | 89 | -1 |
| `prodex-app/src` Rust files | 716 | 697 | -19 |
| `prodex-app/src` physical lines | 203,074 | 198,219 | -4,855 (-2.39%) |
| `prodex-app/tests` physical lines | 71,682 | 68,590 | -3,092 (-4.31%) |
| Size-guard allowlist hits | 55 | 54 | -1 |

Every remaining `prodex-app` dependency has an ownership reason in
[the dependency inventory](prodex-app-dependencies.md).

## Build, Binary, And Startup

| Metric | Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| Clean release build wall time | 233.73 s | 235.86 s | +0.91% |
| Clean release build user time | 1,646.84 s | 1,687.88 s | +2.49% |
| Clean release build system time | 97.16 s | 103.79 s | +6.82% |
| Clean release build max RSS | 2,928,264 KiB | 2,851,952 KiB | -2.61% |
| Release binary | 54,153,624 bytes | 45,984,896 bytes | -8,168,728 (-15.08%) |
| ELF text | 40,328,863 bytes | 44,989,451 bytes | +11.56% |
| First startup | 5.895 ms | 4.577 ms | -22.35% |
| Warm startup p50 | 3.936 ms | 4.015 ms | +2.01% |
| Warm startup p95 | 6.128 ms | 5.906 ms | -3.63% |
| Warm startup p99 | 6.607 ms | 6.340 ms | -4.04% |
| Startup RSS p50 | 7,480 KiB | 8,196 KiB | +9.57% |
| Startup RSS p95 | 7,676 KiB | 8,372 KiB | +9.07% |
| Startup RSS p99 | 7,728 KiB | 8,444 KiB | +9.27% |

The binary reduction combines symbol stripping, bounded tokenizer linkage, and
asset removal. The isolated Caveman removal comparison was 54,142,968 bytes
before and 54,079,328 bytes after, a direct 63,640-byte reduction; the embedded
developer instruction was present before and absent after.

Startup latency stayed inside the 5% guard. Startup RSS did not: the controlled
median increased by 716 KiB. ELF writable data grew by about 236 KiB and the
expanded typed CLI/runtime surface touches more pages during Clap startup, but
the measurement does not isolate the remaining pages. This is a reported
regression and follow-up item, not a safety or speed claim.

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

## Reproduction Commands

```bash
/usr/bin/time -v cargo build --release --locked
npm run smart-context:replay
npm run docs:smart-context-evidence:check
cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths -- \
  runtime_smart_context_ --noplot --warm-up-time 1 --measurement-time 2 --sample-size 50
npm run bench:smart-context-report
```

Full gate outcomes are recorded in [Testing](../testing.md) after the final
suite. Environmental or repository failures remain failures until explicitly
resolved; unmeasured values are not inferred.
