# Runtime Proxy Load Harness

Load assets for local runtime proxy checks and the lightweight CI smoke.

Use `tests/load/mock-upstream.mjs` as a ChatGPT-like upstream and `tests/load/runtime-proxy-load.mjs` as the load driver. Scenario defaults live in `tests/load/scenarios.json`.

Common commands:

```bash
npm run load:runtime-proxy -- --scenario baseline --start-mock
npm run load:runtime-proxy -- --scenario baseline --start-mock --start-proxy --prodex ./target/debug/prodex
npm run load:runtime-proxy -- --scenario slow-client --start-mock --start-proxy --prodex ./target/debug/prodex
npm run load:runtime-proxy -- --scenario slow-upstream --start-mock --start-proxy --prodex ./target/debug/prodex
npm run load:runtime-proxy -- --scenario long-stream --start-mock --start-proxy --prodex ./target/debug/prodex
npm run load:runtime-proxy -- --scenario stress --start-mock --start-proxy --prodex ./target/debug/prodex --profiles 4
npm run load:runtime-proxy -- --scenario spike --target http://127.0.0.1:9901/backend-api
npm run load:self-test
npm run load:runtime-proxy -- --scenario long-stream --dry-run
```

The driver reports request error rate, TTFT percentiles, latency percentiles, status mix, route mix, and admission-pressure evidence from local responses plus runtime log markers when `--runtime-log-dir` or `--start-proxy` is used. The slow-client case delays response-body reads, slow-upstream delays every first byte and chunk, and long-stream emits 64 ordered output deltas. Scenario validation caps concurrency, duration/request count, delays, per-request timeout, chunk count, chunk size, and total mock stream bytes.

Every summary also reports `performance_evidence`. Allocation/request is
unsupported because the harness has no allocation counter. With `--start-proxy`,
the harness reads the authenticated broker metrics endpoint before and after the
load and reports admission wait, long-lived queue wait, and runtime-state lock
wait separately. Each captured item includes cumulative start/end snapshots and
a process-level delta with total nanoseconds, wait count, the cumulative maximum
at the end of the run, and mean nanoseconds per newly observed wait. The maximum
counter cannot be differenced into an interval maximum, and the final read-only
metrics snapshot contributes to runtime-state lock wait. Direct-target and
mock-only runs report these broker-only durations as not captured; pressure
markers remain pressure evidence rather than queue-latency estimates.

CI smoke:

```bash
npm run ci:runtime-load-smoke
```

The CI smoke runs a small mock-only baseline with zero tolerated request errors and admission pressure, plus a bounded p95 TTFT threshold. It does not build or launch `prodex`, so it is cheap enough for scheduled/manual CI and heavy PR paths. For local proxy coverage after `cargo build`, use:

```bash
npm run ci:runtime-load-smoke -- --mode proxy --prodex ./target/debug/prodex
```
