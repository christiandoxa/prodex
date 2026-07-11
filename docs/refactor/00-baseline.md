# Refactor Baseline

## Snapshot

- Captured: 2026-07-11, Asia/Jakarta.
- Git branch: `main`.
- Git commit: `14e862f72a7a5cfd1a6c5828271c52cd13962bce`.
- Worktree before refactor: clean (`git status --short` produced no output).
- Package version: `0.276.0`.
- Workspace inventory: 61 Cargo manifests, 1,655 Rust files, and 465,724 Rust lines.

This baseline separates pre-existing failures from refactor regressions. A command marked
`pending` has not produced authoritative evidence yet and must not be treated as passing.

## Environment

| Item | Value |
| --- | --- |
| OS | Zorin OS 18.1 (`noble`) |
| Kernel | Linux 6.17.0-35-generic x86_64 |
| CPU | AMD Ryzen 5 PRO 4650G, 6 cores / 12 threads |
| Memory | 30 GiB RAM, 2 GiB swap |
| Rust | `rustc 1.97.0 (2d8144b78 2026-07-07)` |
| Cargo | `cargo 1.97.0 (c980f4866 2026-06-30)` |
| Node.js | `v24.18.0` |
| npm | `12.0.1` |
| Declared npm package manager | `npm@11.12.1` |

The installed npm version differs from `package.json`. Any npm failure must be checked for
tool-version sensitivity before it is attributed to repository code.

## Required Command Baseline

No command below may be removed because it is slow or fails. Environmental failures are
recorded with their exact cause.

| Command | Status | Evidence |
| --- | --- | --- |
| `git status --short` | pass | No output before edits |
| `cargo metadata --locked --format-version 1` | pass | 0.307 s |
| `cargo tree --workspace -d` | pass | 0.121 s; duplicate families inventoried below |
| `cargo fmt --check` | pass | 2.919 s |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | pass | 91.506 s |
| `cargo test --locked --workspace --all-features` | pre-existing failure | 90.006 s; two concurrent cache-stat assertions described below |
| `npm ci` | environmental failure | 0.206 s; repository has no `package-lock.json` |
| `npm test` | pass | 206.428 s |
| `npm run docs:lint` | pass | 0.324 s |
| `npm run ci:crate-boundary` | pass | 0.116 s |
| `npm run ci:domain-boundary-guard` | pass | 0.131 s |
| `npm run ci:application-boundary-guard` | pass | 0.135 s |
| `npm run ci:auth-boundary-guard` | pass | 0.121 s |
| `npm run ci:gateway-core-boundary-guard` | pass | 0.121 s |
| `npm run ci:gateway-http-boundary-guard` | pass | 0.133 s |
| `npm run ci:deployment-security-guard` | pass | 0.132 s |
| `npm run ci:dependency-duplicates` | pass | 0.315 s; 16/16 duplicate families within budget |
| `cargo audit` | pass | 2.284 s; 496 locked dependencies, no reported advisories |
| `cargo deny check advisories sources` | pass | 2.420 s |

Additional local gates passed:

- `npm run ci:runtime-hotpath-guard` in 0.209 s;
- `npm run ci:runtime-load-smoke` in 3.067 s with 32/32 successful requests, no
  admission-pressure errors, and contaminated TTFT p95 of 75.76 ms;
- `cargo bench --locked --features bench-support --bench runtime_proxy_hot_paths` in
  206.896 s as a functional-only run; and
- `npm run ci:runtime-stress` in 207.544 s across the broad 442-test runtime set and
  serialized/continuation selections.

The full workspace test failure was limited to:

- `continuity_failure_reason_metrics_reuse_cached_summary_for_unchanged_log`; and
- `continuity_failure_reason_metrics_incrementally_update_when_log_appends`.

Both compared process-global cache statistics while the workspace ran concurrently. A focused
serial rerun, `cargo test --locked -p prodex-runtime-broker-log -- --test-threads=1`, passed 3/3 in
0.445 s. The baseline therefore records this as test isolation/interference, not a behavior failure
to conceal during the refactor.

## Structural Inventory

The codebase-memory index at the snapshot contained 31,407 nodes and 160,179 edges with no
reported skipped or partially parsed source files. Documentation, deployment assets, scripts,
fixtures, generated targets, and several asset directories are excluded by that index, so direct
file inspection remains authoritative for those paths.

Initial boundary evidence:

- `prodex-application::plan_application_request_authentication` has only application-crate test
  call sites in indexed Rust code. Production wiring is not yet proven.
- `prodex-gateway-server::route_allowed` allows every non-control route in data-plane mode,
  including `GatewayHttpRouteKind::Unknown`.
- `prodex-gateway-http::classify_route` recognizes only responses, compact, realtime, quota,
  health, and broad control-plane mounts. Other documented provider routes become `Unknown`.
- `prodex-gateway-http` currently trims request targets, strips query/fragment text, collapses
  empty segments during control-route planning, and constructs separate canonical strings.
- `prodex expose` still exposes `ExposeArgs::no_tunnel`; tunnel safety and session transport need
  Phase 1 regression coverage before behavior changes.
- Kubernetes currently launches the legacy root `prodex gateway` command directly. Only the
  dedicated enterprise binaries compose `prodex-gateway-server`, so fixing that front alone does
  not secure the deployed path.
- The legacy handler in `prodex-app` still owns the combined health/admin auth, virtual-key auth,
  admission, guardrail, accounting, routing, and provider-dispatch lifecycle.

Cargo found no dependency cycle. Critical layering remains server -> HTTP -> core/domain,
application -> boundary crates, app -> application/HTTP, and root -> composition. One accidental
upward candidate is `prodex-session-store -> prodex-app-reports`, which the generic low-level crate
guard does not currently cover.

Workspace package aliases duplicate `prodex-runtime-anthropic` twice and
`prodex-runtime-proxy` three times. External duplicate families reported by Cargo include
`block-buffer`, `cfg_aliases`, `const-oid`, `cpufeatures`, `crypto-common`, `digest`,
`fallible-iterator`, `getrandom`, `hashbrown`, `itertools`, `nix`, `rand`, `rand_core`,
`thiserror`, and `untrusted`. The repository duplicate guard accepts all current budgets; removal
requires call-site and compatibility proof rather than blind lockfile churn.

## Existing Compatibility Nets

The following suites and fixtures are baseline contracts, not implementation details to delete:

- provider conformance under `crates/prodex-provider-core/tests/`;
- compatibility replay fixtures under `crates/prodex-app/tests/fixtures/compat_replay/`;
- runtime proxy integration coverage under `crates/prodex-app/tests/src/runtime_proxy/` and its
  support modules;
- gateway HTTP policy tests under `crates/prodex-gateway-http/tests/http_policy.rs`;
- gateway server streaming, upgrade, admission, and drain tests;
- application, authn, authz, storage, deployment, and crate-boundary guards in `package.json`.

## Baseline Rules for Later Phases

1. Add a failing contract test before each security behavior change.
2. Preserve upstream response/status/stream transparency once an upstream response exists.
3. Preserve hard continuation affinity and prohibit rotation after stream commitment.
4. Do not compare performance samples taken during builds, tests, or unrelated load.
5. Attribute every later failure as pre-existing, environmental, or introduced, with evidence.
