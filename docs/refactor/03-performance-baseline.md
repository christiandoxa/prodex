# Performance Baseline

## Rule

No performance claim is valid without comparable repeated samples. Security, backpressure,
streaming, affinity, and accounting correctness override throughput gains.

## Snapshot Environment

| Item | Value |
| --- | --- |
| Commit | `14e862f72a7a5cfd1a6c5828271c52cd13962bce` |
| OS/kernel | Zorin OS 18.1; Linux 6.17.0-35-generic x86_64 |
| CPU | AMD Ryzen 5 PRO 4650G; 6 cores / 12 threads; boost enabled |
| RAM | 30 GiB |
| Rust | 1.97.0 |
| Build profile | Criterion default unless a command states otherwise |

## Method

1. Build once before sampling.
2. Stop unrelated builds, tests, load generators, and high-CPU work.
3. Record CPU governor/frequency state and available memory.
4. Run at least five samples with the same command and configuration.
5. Preserve raw Criterion JSON/load output outside prose summaries.
6. Report median, p95/p99 where the harness provides them, throughput, CPU/request, RSS,
   allocation/request, queue wait, and lock wait. Mark unsupported metrics explicitly rather than
   inventing values.
7. Use a default no-regression guard of 5% after variance analysis.

## Existing Local Gates

```bash
cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths
PRODEX_RUNTIME_PROXY_BENCH_CHECK=1 cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths
npm run load:runtime-proxy
npm run ci:runtime-load-smoke
npm run ci:runtime-stress
```

The Criterion benchmark covers quota fallback scanning, previous-response selection, mixed-pool
selection, compact session affinity, WebSocket stale reuse, SSE lookahead inspection, dead-lineage
cleanup, and large smart-context output rewriting. Its explicit gate reports median and p90
nanoseconds; it does not by itself prove end-to-end p95/p99, CPU, RSS, allocations, queue wait, or
lock wait.

## Raw Baseline Samples

One Criterion functional run completed all eight targets and the runtime load smoke completed
32/32 requests. They ran alongside baseline activity, so their timings are retained only as
functional evidence. The observed load-smoke TTFT p95 was 75.76 ms; it is not a comparative
baseline.

Five isolated hot-path check repetitions were then run directly from the warmed release binary in
the clean baseline worktree. Values below are nanoseconds per iteration; `median-5` is the median
of the five independently calibrated runs and CV is the sample coefficient of variation of those
five reported medians.

| Case | Median samples | Median-5 | CV | P90 median-5 | P90 CV |
| --- | --- | ---: | ---: | ---: | ---: |
| Quota fallback scan | 23,538 / 23,198 / 23,358 / 23,083 / 23,350 | 23,350 | 0.74% | 23,393 | 1.91% |
| Previous-response selection | 100,127 / 120,072 / 104,224 / 115,255 / 104,886 | 104,886 | 7.68% | 113,984 | 7.77% |
| Mixed-pool response selection | 1,465,335 / 1,489,257 / 1,487,207 / 1,486,948 / 1,492,787 | 1,487,207 | 0.73% | 1,500,480 | 0.87% |
| Compact session affinity | 8,524 / 8,514 / 9,637 / 11,219 / 9,749 | 9,637 | 11.68% | 10,346 | 13.87% |
| WebSocket stale reuse | 9,952 / 11,143 / 11,036 / 10,700 / 11,252 | 11,036 | 4.86% | 11,193 | 3.38% |
| SSE lookahead | 106,140 / 105,678 / 106,512 / 106,459 / 107,714 | 106,459 | 0.71% | 106,818 | 0.74% |
| Dead-lineage cleanup | 137,548 / 136,122 / 136,936 / 137,041 / 138,046 | 137,041 | 0.53% | 138,531 | 0.93% |
| Large tool-output rewrite | 1,657,031 / 1,604,467 / 1,601,736 / 1,622,185 / 1,622,198 | 1,622,185 | 1.36% | 1,624,123 | 1.62% |

The previous-response case exceeded its fixed 110,000 ns median threshold in samples two and four.
That is baseline flakiness, not a refactor regression; the matched comparison therefore uses the
five-run distribution and the default 5% delta guard rather than hiding those failures.

The matched end-to-end baseline used the debug binary, two synthetic profiles, the bounded mock
upstream, 120 requests, and concurrency eight. GNU `time` measured the complete harness process
tree, including startup and teardown. Consequently CPU/request and peak RSS are comparative harness
metrics, not isolated proxy-process claims.

```bash
/usr/bin/time -f 'resource wall_s=%e user_s=%U sys_s=%S max_rss_kib=%M' \
  node tests/load/runtime-proxy-load.mjs \
  --scenario baseline --start-mock --start-proxy --prodex target/debug/prodex \
  --requests 120 --concurrency 8 --max-error-rate 0.02 \
  --max-admission-pressure-rate 0.05 --max-ttft-p95-ms 1500
```

| Metric | Five samples | Median-5 | CV |
| --- | --- | ---: | ---: |
| TTFT p95 (ms) | 107.03 / 106.46 / 96.10 / 98.88 / 100.88 | 100.88 | 4.68% |
| TTFT p99 (ms) | 214.48 / 195.75 / 178.27 / 188.13 / 185.53 | 188.13 | 7.18% |
| Completion p95 (ms) | 131.23 / 129.41 / 121.73 / 126.75 / 126.45 | 126.75 | 2.83% |
| Completion p99 (ms) | 240.72 / 223.95 / 203.76 / 213.45 / 211.02 | 213.45 | 6.56% |
| Throughput (requests/s) | 7.353 / 7.376 / 7.376 / 7.362 / 7.371 | 7.371 | 0.13% |
| CPU/request (ms) | 123.00 / 122.83 / 121.25 / 122.67 / 122.08 | 122.67 | 0.58% |
| Peak harness RSS (KiB) | 92,784 / 93,816 / 92,156 / 92,784 / 92,072 | 92,784 | 0.75% |

### Baseline evidence completeness

The preserved five runs did not capture every metric required by the method.
Missing values are not inferred from RSS, elapsed time, or log-marker counts.

| Required metric | Before evidence | Existing read-only source | Baseline interpretation |
| --- | --- | --- | --- |
| Allocation/request | unsupported; not captured in any sample | none in the Criterion or load harness | no allocation no-regression claim |
| Queue wait | unsupported; not captured in any sample | runtime logs expose some recovered/exhausted waits, but not every successful wait | admission-pressure markers are not queue-wait latency; no queue-wait no-regression claim |
| Runtime-state lock wait | not captured in any sample | authenticated broker metrics expose cumulative total/count/max | the baseline command did not snapshot the endpoint, so no lock-wait no-regression claim |

The current load harness can take read-only broker lock-wait snapshots for new
`--start-proxy` runs. That does not retroactively supply values for these
historical samples, and no replacement baseline was run for this documentation
update.

All 600 requests succeeded. Sample two emitted one `profile_inflight_saturated` marker without an
error response; the other four emitted no admission-pressure marker. This is retained as baseline
backpressure variance.

| Case | Sample 1 | Sample 2 | Sample 3 | Sample 4 | Sample 5 | Variance/status |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Runtime hot-path check | pass | one threshold miss | pass | one threshold miss | pass | complete; distribution above |
| Runtime proxy load | pass | pass with one pressure marker | pass | pass | pass | complete; 600/600 responses succeeded |
| Slow client load scenario | not captured | not captured | not captured | not captured | not captured | implemented and self-tested; no matched baseline samples |
| Slow upstream load scenario | not captured | not captured | not captured | not captured | not captured | implemented and self-tested; no matched baseline samples |
| High concurrency/overload load scenarios | not captured | not captured | not captured | not captured | not captured | stress/spike runs are reported in the results file, but not as matched five-sample evidence |
| Long-stream load scenario | not captured | not captured | not captured | not captured | not captured | implemented and self-tested; no matched baseline samples |
| Soak load scenario | not captured | not captured | not captured | not captured | not captured | implemented; no final matched samples |
| Cancellation, partial stream, upgrade drain, deterministic shutdown | n/a | n/a | n/a | n/a | n/a | focused correctness tests only; not load-performance evidence |

## Acceptance Comparison

`docs/refactor/04-performance-results.md` will contain the matched post-change samples, deltas,
variance, interpretation, and any rejected optimization. Until that file has comparable evidence,
the project makes no performance-improvement claim.
