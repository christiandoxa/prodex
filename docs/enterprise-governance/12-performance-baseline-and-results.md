# Performance Baseline and Results

This is the evidence template for governance performance. No new governance
benchmark result is claimed here: the required controlled five-sample runs have
not yet been recorded. Historical repository measurements may inform harness
design but do not satisfy this program's acceptance gate. Detailed methodology
is in
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

## Result register

| Case | Revision/machine | Samples | Result | Status |
| --- | --- | --- | --- | --- |
| Disabled compatible baseline | Not recorded | 0 | Not measured | pending |
| Governance disabled delta | Not recorded | 0 | Not measured | pending |
| End-to-end governed overhead | Not recorded | 0 | Not measured | pending |
| Local PDP | Not recorded | 0 | Not measured | pending |
| Governed routing | Not recorded | 0 | Not measured | pending |

A result becomes `pass` only when raw artifacts, exact commands, revisions and
machine metadata are linked and the functional test suite passes on the same
candidate.
