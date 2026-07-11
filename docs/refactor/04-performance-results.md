# Performance Results

## Scope and interpretation

These results compare the clean `14e862f72a7a5cfd1a6c5828271c52cd13962bce` worktree with
the completed refactor on the same host. The CPU governor was `performance`, frequency boost was
enabled, and unrelated builds were stopped during samples. Each comparison uses five independent
runs and the default 5% no-regression guard after variance analysis.

| Item | Value |
| --- | --- |
| Host | AMD Ryzen 5 PRO 4650G, 6 cores / 12 threads, 30 GiB RAM |
| OS | Zorin OS 18.1; Linux 6.17.0-35-generic x86_64 |
| Rust | 1.97.0 |
| CPU policy | `acpi-cpufreq`, `performance`, boost enabled |
| Full baseline method/raw data | [03-performance-baseline.md](03-performance-baseline.md) |

Security, bounded admission, continuation affinity, stream commitment, and accounting correctness
remain release blockers regardless of throughput. The report makes no broad optimization claim:
architectural/security work dominates this change, and only measured paths are discussed.

## Matched hot-path check

The warmed release benchmark binary runs seven internally sampled measurements per case and reports
median and p90 nanoseconds per iteration. Five outer repetitions are compared here. The exact build
and run command is:

```bash
PRODEX_RUNTIME_PROXY_BENCH_CHECK=1 \
  cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths
```

| Case | Before median-5 (ns) | After median-5 (ns) | Delta | Before CV | After CV | Result |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Quota fallback scan | 23,350 | 16,428 | -29.64% | 0.74% | 1.74% | improved |
| Previous-response selection | 104,886 | 93,272 | -11.07% | 7.68% | 6.27% | improved |
| Mixed-pool response selection | 1,487,207 | 1,490,205 | +0.20% | 0.73% | 1.09% | no regression |
| Compact session affinity | 9,637 | 9,313 | -3.36% | 11.68% | 1.96% | improved |
| WebSocket stale reuse | 11,036 | 9,554 | -13.43% | 4.86% | 1.21% | improved |
| SSE lookahead | 106,459 | 105,191 | -1.19% | 0.71% | 1.06% | improved |
| Dead-lineage cleanup | 137,041 | 142,135 | +3.72% | 0.53% | 2.55% | within guard |
| Large tool-output rewrite | 1,622,185 | 1,553,484 | -4.24% | 1.36% | 0.50% | improved |

The baseline's previous-response fixed threshold missed in two of five outer runs. Conclusions use
the full distribution and matched delta, not a single threshold result.

Post-change median samples, in case-table order, were:

```text
quota:     16454 16349 16428 16967 16211
previous:  97662 83729 87200 94898 93272
mixed:     1485844 1505875 1511110 1470655 1490205
compact:   9305 9262 9680 9561 9313
websocket: 9323 9554 9586 9607 9553
sse:       105185 105191 104287 106717 106909
cleanup:   139022 142135 138508 145849 146153
rewrite:   1545343 1565838 1551139 1553484 1558584
```

All five post-change outer runs passed every fixed threshold. During calibration, the first build
revealed a real `available_parallelism()` call on every probe-queue pressure read. Adding the
initialized-queue fast path removed that syscall from the hot path; the passing samples above were
collected only after that fix. Median-of-five p90 deltas were -28.95%, -11.09%, -0.08%, -9.61%,
-14.12%, -0.83%, +4.06%, and -3.74%, respectively.

## Matched end-to-end proxy load

The debug-binary load comparison uses a bounded mock upstream, two synthetic profiles, 120 requests,
and concurrency eight. GNU `time` measures the complete harness process tree, including startup and
teardown, so CPU/request and RSS are comparative harness metrics rather than isolated proxy metrics.

```bash
/usr/bin/time -f 'resource wall_s=%e user_s=%U sys_s=%S max_rss_kib=%M' \
  node tests/load/runtime-proxy-load.mjs \
  --scenario baseline --start-mock --start-proxy --prodex target/debug/prodex \
  --requests 120 --concurrency 8 --max-error-rate 0.02 \
  --max-admission-pressure-rate 0.05 --max-ttft-p95-ms 1500
```

| Metric | Before median-5 | After median-5 | Delta | Before CV | After CV | Result |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| TTFT p95 | 100.88 ms | 104.67 ms | +3.76% | 4.68% | 2.05% | within guard |
| TTFT p99 | 188.13 ms | 188.92 ms | +0.42% | 7.18% | 1.72% | no regression |
| Completion p95 | 126.75 ms | 126.79 ms | +0.03% | 2.83% | 4.09% | no regression |
| Completion p99 | 213.45 ms | 215.37 ms | +0.90% | 6.56% | 1.36% | no regression |
| Throughput | 7.371 requests/s | 7.417 requests/s | +0.62% | 0.13% | 0.33% | improved |
| CPU/request | 122.67 ms | 120.58 ms | -1.70% | 0.58% | 0.32% | improved |
| Peak harness RSS | 92,784 KiB | 92,368 KiB | -0.45% | 0.75% | 1.02% | improved |

All 600 baseline requests succeeded. One run emitted one `profile_inflight_saturated` marker without
an error response; the other four emitted no admission-pressure marker.

All 600 post-change requests also succeeded. One run emitted one admission-pressure log marker
without an admission-pressure response; the other four emitted none. Post-change raw samples were:

```text
TTFT p95 ms:       100.62 106.24 104.67 103.12 104.75
TTFT p99 ms:       183.41 191.13 188.92 186.70 190.94
completion p95 ms: 123.21 137.19 128.32 126.50 126.79
completion p99 ms: 210.31 217.96 215.37 214.14 216.61
wall seconds:      16.16 16.18 16.10 16.24 16.21
user seconds:      13.83 13.93 13.87 13.98 13.95
system seconds:    0.61 0.54 0.54 0.54 0.56
peak RSS KiB:      93936 93464 92036 91748 92368
```

The harness now starts the internal broker through the same bounded stdin bootstrap used by
production and creates private synthetic credential files. This preserves the no-secret-argv/env
contract instead of benchmarking a removed compatibility path.

## Allocator decision evidence

The duplicated Linux/glibc `malloc_trim` calls were centralized and restricted to the existing
rate-limited large buffered-release path. Five release-process samples allocated 16,384 buffers of
16 KiB (about 256 MiB). RSS immediately after drop and after the gated trim was:

| Sample | After drop (KiB) | After trim (KiB) | Reclaimed (KiB) |
| ---: | ---: | ---: | ---: |
| 1 | 2,244 | 2,116 | 128 |
| 2 | 2,240 | 2,112 | 128 |
| 3 | 2,244 | 2,116 | 128 |
| 4 | 2,208 | 2,080 | 128 |
| 5 | 2,240 | 2,112 | 128 |

The repeatable but small reclaim justifies retaining the centralized, rate-limited call. It does
not justify unconditional trims: four background-path calls were removed.

## Export KDF calibration

Five release samples per setting measured Argon2id v19 with parallelism one and three iterations:

| Memory | Mean | Standard deviation | Decision |
| ---: | ---: | ---: | --- |
| 32 MiB | 64.726 ms | 1.074 ms | compatibility calibration only |
| 64 MiB | 130.267 ms | 2.566 ms | selected v2 default |
| 128 MiB | 263.148 ms | 2.577 ms | accepted bounded import setting, not default |

Version 2 therefore uses 64 MiB, three iterations, and parallelism one. Version 1 PBKDF2 imports
remain bounded and compatible; no KDF parameter is silently reinterpreted.

## Reliability scenarios

The checked-in stress and spike scenarios were also run against both binaries. Their configured
raw-marker thresholds already fail on the clean baseline because every repeated lane/profile probe
log line is counted as a separate pressure event; these failures were not waived or converted into
passes.

| Run | Success | TTFT p95 | Pressure rate | Gate result |
| --- | ---: | ---: | ---: | --- |
| Baseline stress, default bounds | 776/800 | 1,593.69 ms | 0.9588 | failed TTFT and raw-marker thresholds |
| Refactor stress, default bounds (run 1) | 780/800 | 1,895.44 ms | 0.9038 | failed TTFT and raw-marker thresholds |
| Refactor stress, default bounds (run 2) | 774/800 | 1,839.42 ms | 1.2862 | failed error, TTFT, and raw-marker thresholds |
| Baseline spike, default bounds | 470/500 | 2,376.75 ms | 0.5420 | failed error and raw-marker thresholds |
| Refactor spike, default bounds | 477/500 | 2,457.66 ms | 0.5260 | failed raw-marker threshold only |
| Refactor stress, explicit bounded capacity calibration | 796/800 | 560.04 ms | 0 | passed |

The calibrated run retained the global active-request bound while setting per-profile soft/hard
limits to 16/32 and the compact lane to 12 for the three-profile, concurrency-64 fixture. That run
used:

```bash
PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT=16 \
PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT=32 \
PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT=12 \
  node tests/load/runtime-proxy-load.mjs --scenario stress \
  --start-mock --start-proxy --prodex target/debug/prodex
```

This is capacity evidence, not a default change: the production defaults remain conservative and
bounded. The matched five-run baseline scenario above is the authoritative no-regression
comparison. Slow upstreams, cancellation, partial streams, upgrade drain, and deterministic
shutdown are additionally covered by focused gateway/runtime/storage tests.
