# Performance Baseline and Results

This records the release-candidate governance and compatibility measurements.
Detailed methodology is in
[`12-testing-performance-and-evidence.md`](12-testing-performance-and-evidence.md).

## Acceptance budgets

| Case | Budget |
| --- | --- |
| Governance disabled | No more than 5% p95/p99 latency or throughput regression versus the same revision's compatible baseline |
| End-to-end governed local overhead | At most 5 ms p99 excluding external provider/inspection time |
| Local PDP | At most 1 ms p99 |
| Governed routing | At most 2 ms p99 |

Functional security and correctness gates take precedence over these budgets.
An optimization cannot remove required validation, audit or fail-closed
behavior.

## Measurement protocol

1. Pin the commit, release build, feature/mode, configuration and dependency
   revisions.
2. Record CPU model/count, memory, OS/kernel, Rust version and power/container
   limits; do not record hostnames or personal paths.
3. Use synthetic content and deterministic provider/inspection stubs so remote
   variance is separately reported.
4. Warm caches explicitly, then interleave before/after cases.
5. Collect at least five independent comparable samples and preserve raw output.
6. Report sample values, median, p50/p95/p99, throughput, errors, allocation/RSS
   where relevant, and variance/outliers.
7. Run enough concurrency to expose admission, snapshot refresh, outbox and
   session contention without relying on unbounded work.

## Required benchmark cases

- request capture and schema-aware inspection;
- classification and PDP evaluation (cold compile outside hot path, warm read);
- obligation merge and provider hard-filter/fixed-point score;
- snapshot load/refresh/invalidation;
- audit append/hash and SIEM outbox claim/ack;
- session validation/revocation lookup;
- stream inspection at representative chunk boundaries;
- end-to-end `personal`, observe, enterprise and bank modes; and
- multi-replica load, failover and degraded dependencies.

Executable Criterion coverage for maximum bounded inspection findings, the
maximum compiled PDP rule count, and the maximum governed-provider candidate
count is provided by:

```bash
cargo bench --locked --bench governance_hot_paths
```

This is a hot-path microbenchmark, not end-to-end or operational SLO evidence.

## Candidate microbenchmark evidence

Recorded on 2026-07-13 from the release candidate based on `e308fdf6`, using
Linux 6.17, an AMD Ryzen 5 PRO 4650G (6 cores/12 threads), and 30 GiB RAM. The
exact commands were:

```bash
cargo bench --bench governance_hot_paths
/usr/bin/time -v target/release/deps/governance_hot_paths-* --bench --noplot
```

Quantiles below are calculated from Criterion's 100 per-iteration samples. The
same timed run used 109% CPU and 24,200 KiB peak RSS. Allocation, queue-wait and
lock-wait instrumentation is not present in this pure single-thread
microbenchmark; those values are therefore not claimed.

| Maximum-bound case | p50 | p95 | p99 | p50 throughput | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| Inspection result, maximum findings | 26.926 us | 27.755 us | 28.221 us | 37,139/s | pass |
| PDP, 256 compiled rules | 1.963 us | 2.043 us | 2.066 us | 509,422/s | pass |
| Governed routing, 64 candidates | 5.734 us | 6.069 us | 6.530 us | 174,411/s | pass |

These results prove bounded pure-stage cost, not external Presidio/provider or
multi-replica SLO compliance.

## Governance-disabled compatibility evidence

The clean `e308fdf6` checkout and candidate were built separately, pinned to
the same CPU, warmed, and interleaved. Each displayed row uses 100 Criterion
samples. Percentages are candidate versus baseline. Redundant per-candidate
file log writes were removed; the single bounded content-free route decision
trace remains authoritative.

```bash
taskset -c 2 <baseline-binary> --bench --noplot
taskset -c 2 <candidate-binary> --bench --noplot
```

| Disabled compatibility case | Baseline p50/p95/p99 | Candidate p50/p95/p99 | Delta p50/p95/p99 | Status |
| --- | --- | --- | --- | --- |
| Quota fallback scan | 18.199/18.677/19.423 us | 13.239/14.050/14.852 us | -27.3%/-24.8%/-23.5% | pass |
| Previous-response selection | 165.228/190.223/222.560 us | 10.912/12.540/15.608 us | -93.4%/-93.4%/-93.0% | pass |
| Mixed-pool selection | 1.546/1.616/1.696 ms | 1.538/1.602/1.734 ms | -0.5%/-0.8%/+2.3% | pass |
| Compact session affinity | 15.865/18.314/21.573 us | 15.773/18.176/21.605 us | -0.6%/-0.7%/+0.1% | pass |
| WebSocket stale reuse | 16.620/18.771/20.642 us | 16.009/17.967/21.543 us | -3.7%/-4.3%/+4.4% | pass |
| SSE lookahead | 101.806/110.727/122.260 us | 105.104/109.939/122.275 us | +3.2%/-0.7%/+0.0% | pass |
| Dead-lineage cleanup | 157.431/176.170/208.883 us | 159.850/167.556/177.884 us | +1.5%/-4.9%/-14.8% | pass |
| Smart-context rewrite | 1.610/1.939/2.361 ms | 1.643/1.738/1.802 ms | +2.0%/-10.4%/-23.7% | pass |

All disabled-path p95 and p99 deltas remain within the 5% ceiling.

## Load and resource evidence

The deterministic load self-test and local smoke workload passed 32/32
requests with 68.56 ms p95 time-to-first-token. Runtime stress passed after
fixture-only permission and synthetic-hash corrections. The governance fuzz
target completed 117,401 executions in 31 seconds with no finding. These runs
do not claim external-provider capacity, multi-replica soak, or production CPU,
RSS, allocation, queue-wait, and lock-wait SLOs.

## Result register

| Case | Revision/machine | Samples | Result | Status |
| --- | --- | --- | --- | --- |
| Disabled compatible baseline | `e308fdf6`, machine above | 100/case | Proxy hot-path baseline captured | tested |
| Governance disabled delta | Candidate, machine above | 100/case | Worst p99 delta +4.4% | tested |
| End-to-end local smoke | Candidate, machine above | 32 | 32/32; 68.56 ms p95 TTFT | tested |
| Governed external/multi-replica overhead | Candidate, machine above | 0 | Environment-specific acceptance remains required | not claimed |
| Local PDP | Candidate, machine above | 100 | p50 1.963 us; p99 2.066 us | tested |
| Governed routing | Candidate, machine above | 100 | p50 5.734 us; p99 6.530 us | tested |
