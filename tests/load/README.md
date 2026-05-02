# Runtime Proxy Load Harness

Manual load assets for local runtime proxy checks. Nothing here is wired into CI.

Use `tests/load/mock-upstream.mjs` as a ChatGPT-like upstream and `tests/load/runtime-proxy-load.mjs` as the load driver. Scenario defaults live in `tests/load/scenarios.json`.

Common commands:

```bash
npm run load:runtime-proxy -- --scenario baseline --start-mock
npm run load:runtime-proxy -- --scenario baseline --start-mock --start-proxy --prodex ./target/debug/prodex
npm run load:runtime-proxy -- --scenario stress --start-mock --start-proxy --prodex ./target/debug/prodex --profiles 4
npm run load:runtime-proxy -- --scenario spike --target http://127.0.0.1:9901/backend-api
```

The driver reports request error rate, TTFT percentiles, latency percentiles, status mix, route mix, and admission-pressure evidence from local responses plus runtime log markers when `--runtime-log-dir` or `--start-proxy` is used.
