# Hardening Baseline

This baseline was captured before the Caveman and Smart Context hardening work.
It records commands that actually ran on commit
`c635d485cf750637512bfacb4e1cefc854ed1bef`; missing measurements are not
treated as passes.

## Host

| Field | Value |
| --- | --- |
| Date | 2026-07-25 (Asia/Jakarta) |
| OS | Linux 7.0.0-28-generic, x86_64 |
| CPU | AMD Ryzen 7 8845HS, 8 cores / 16 threads |
| Memory | 29 GiB |
| Rust | rustc 1.97.0 (`2d8144b78`), cargo 1.97.0 |
| Node/npm | Node 24.18.0, npm 12.0.1 |
| RTK | 0.43.0 |

The branch was `main`; the worktree and `git diff --check` were clean before
characterization tests were added.

## Structure

| Metric | Baseline |
| --- | ---: |
| Workspace packages (`cargo metadata --locked`) | 61 |
| Rust files outside ignored target trees | 2,020 |
| Rust physical lines outside ignored target trees | 555,876 |
| Crate `src/` physical lines | 446,705 |
| `prodex-app` direct dependencies | 90 |
| `prodex-app/src` Rust files | 716 |
| `prodex-app/src` physical lines | 203,074 |
| `prodex-app/tests` Rust files | 230 |
| `prodex-app/tests` physical lines | 71,682 |
| `prodex-caveman-assets/src` physical lines | 2,766 |
| Size-guard allowlist hits | 55 |

The direct dependency list was captured with:

```bash
cargo metadata --locked --no-deps --format-version 1
```

## Build And Startup

| Measurement | Baseline |
| --- | ---: |
| Clean release build | 124 s |
| Clean `prodex-app` all-target/all-feature check | 45.65 s |
| Clean workspace all-target/all-feature check | 52.28 s |
| Release binary | 54,155,712 bytes |
| Release ELF text | 40,332,959 bytes |
| First `prodex --version` process | 3.727 ms |
| Warm process median, 100 runs | 2.903 ms |
| Warm process p95 / p99 | 3.558 / 3.675 ms |
| Warm-process maximum RSS, 10 runs | 7,712 KiB |

Clean checks used isolated target directories under `/tmp`; the release build
used the normal release target. Startup samples used `process.hrtime.bigint()`
around `spawnSync` and checked every exit status.

## Smart Context

The existing benchmark exposes only one active-rewrite case. Criterion reported
the following generated result for
`runtime_smart_context_large_tool_output_rewrite`:

| Metric | Baseline |
| --- | ---: |
| Median | 1,121,464 ns |
| Mean | 1,126,212 ns |
| 95% confidence interval for mean | 1,122,016–1,130,779 ns |

Raw Criterion files remain under
`target/criterion/runtime_smart_context_large_tool_output_rewrite/new/` for this
checkout. The baseline has no exact, disabled, canary-out, rejected/no-op,
shadow, rehydration, allocation, CPU, queue-wait, or lock-wait benchmark. It
therefore cannot support claims for those paths. Linux `perf` was unavailable
because `perf_event_paranoid=4`.

The documented replay command failed because Cargo could not choose among the
three binaries. Adding `--bin prodex` ran the evaluator, but inspection confirmed
that it aggregated fixture-supplied output metrics. Those values are explicitly
excluded from evidence. No real tokenizer was present, so the baseline has no
verified corpus token counts.

## Runtime Load

`npm run ci:runtime-load-smoke` passed 32/32 requests:

| Metric | Baseline |
| --- | ---: |
| Error rate | 0 |
| TTFT p50 / p95 / p99 | 52.19 / 71.33 / 92.01 ms |
| Completion p50 / p95 / p99 | 111.99 / 130.64 / 150.69 ms |
| Admission-pressure responses | 0 |

The load harness reported allocation/request, broker admission wait, long-lived
queue wait, and runtime-state lock wait as unsupported or not captured.
`npm run ci:runtime-stress` passed.

## Baseline Gates

| Command | Result |
| --- | --- |
| `git diff --check` | pass |
| `cargo metadata --locked --format-version 1` | pass |
| `cargo tree --workspace -d` | pass |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo test --locked --workspace --all-features` | pass: 3,098 tests in 192 suites |
| `npm ci` | **fail (repository defect): root `package-lock.json` absent** |
| `npm test` | pass |
| `npm run docs:lint` | pass |
| `npm run docs:provider-capabilities:check` | pass with Cargo installed |
| `npm run catalog:providers` | pass: 64 models / 7 providers |
| `npm run ci:preflight` | pass |
| `npm run ci:crate-boundary` | pass: 61 packages / 189 edges |
| `npm run ci:runtime-hotpath-guard` | pass: 448 files / 49 allowlist hits |
| `npm run ci:supply-chain-guard` | pass |
| `npm run ci:allow-guard` | pass |
| `npm run ci:super-wildcard-guard` | pass at cap 174 |
| `npm run ci:deployment-security-guard` | pass |
| `cargo audit` | pass: 543 locked dependencies scanned |
| `cargo deny check advisories sources` | pass |

`npm ci` is the only required baseline command that failed. Optional PostgreSQL
execution proof was skipped by the existing preflight because
`PRODEX_TEST_POSTGRES_URL` was not configured.
