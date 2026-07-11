# Runtime Proxy Load Harness

Load assets for local runtime proxy checks and the lightweight CI smoke.

Use `tests/load/mock-upstream.mjs` as a ChatGPT-like upstream and `tests/load/runtime-proxy-load.mjs` as the load driver. Scenario defaults live in `tests/load/scenarios.json`.

Common commands:

```bash
npm run load:runtime-proxy -- --scenario baseline --start-mock
npm run load:runtime-proxy -- --scenario baseline --start-mock --start-proxy --prodex ./target/debug/prodex
npm run load:runtime-proxy -- --scenario stress --start-mock --start-proxy --prodex ./target/debug/prodex --profiles 4
npm run load:runtime-proxy -- --scenario spike --target http://127.0.0.1:9901/backend-api
```

The driver reports request error rate, TTFT percentiles, latency percentiles, status mix, route mix, and admission-pressure evidence from local responses plus runtime log markers when `--runtime-log-dir` or `--start-proxy` is used.

CI smoke:

```bash
npm run ci:runtime-load-smoke
```

The CI smoke runs a small mock-only baseline with zero tolerated request errors and admission pressure, plus a bounded p95 TTFT threshold. It does not build or launch `prodex`, so it is cheap enough for scheduled/manual CI and heavy PR paths. For local proxy coverage after `cargo build`, use:

```bash
npm run ci:runtime-load-smoke -- --mode proxy --prodex ./target/debug/prodex
```
