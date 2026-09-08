# Lean Refactoring Campaign 20260908 Core

## Objective

Reduce proven duplication, dead forwarding, and scattered semantic ownership while preserving
public compatibility, security boundaries, runtime affinity, storage behavior, and provider
transport semantics. This is an implementation campaign, not a line-count exercise.

## Baseline and branch

- `HISTORICAL_RELEASE_BASE`: `e835c1699f0960865c8cacabea6a109bd3500499` (release 0.427.0)
- `CAMPAIGN_BASE`: `afe20dfd9f813eb217f78b63de517c7e8e52e8b7`
- `LAST_EXACT_GREEN_SOURCE_ANCHOR`: `de244d631aa3bca0db4602b323568961c92f2222`
- Baseline ancestry: `origin/main` (`ff7976858c048269d73e970356aa41f4d0b4cafb`) is an ancestor.
- Integration branch: `refactor/parallel-integration-20260908`
- Baseline source: historical `origin/refactor/428-integration-20260907`.
- At the d253 qualification boundary, `origin/refactor/parallel-integration-20260908` resolved to
  `d253232e32f92c56b9f49b9fa68ee00f28d1ac35`; no current integration worktree was registered.
- Main write authorization: not granted. Release hold: true. Checkpoint push scope: campaign branch only.
- Protected main-worktree WIP: `crates/prodex-app/build.rs`;
  `crates/prodex-app/src/runtime_launch/proxy_startup/gemini_sse_state/mojo.rs`;
  `crates/prodex-mojo-core/src/rich/anthropic.rs`;
  `migration/mojo-0.419.2-candidate-backlog.md`;
  `migration/mojo-catalog-ownership.md`; `migration/retry-ownership-0.419.2.md`;
  `mojo/prodex_app/`; `mojo/prodex_core/rich_anthropic.mojo`.

## Status

`CAMPAIGN_PARTIAL`; B's catalog, C's throughput, A's general refactor checkpoints, Wave 4, and
Wave 5 are integrated. Exact CI `34224751563` passed on integration checkpoint
`de244d631aa3bca0db4602b323568961c92f2222` with 72 successful, 3 skipped, and 0 failed jobs; that
SHA is the latest exact-green source anchor. Wave 6 source checkpoints and its audit disposition
are integrated into `d253232e32f92c56b9f49b9fa68ee00f28d1ac35`; its failed exact CI
`34209154761` was repaired by the frozen-expectation checkpoint and integrated as de244. The
next implementation wave is eligible but not started. Provider
catalog and throughput surfaces remain protected. The wider domain audit remains open and this
campaign is partial.

## Parallel ownership ledger

Integration branch: `refactor/parallel-integration-20260908`.

| Worker | Workstream | Base SHA | Branch | Worktree | Owns | Excludes | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A | General lean refactor | `34972926449f8201c925893ae5be3d8e5bb6976c` | `worker/refactor-general-20260908` | worker worktree | CLI, orchestration, profile/auth, session, runtime support, gateway, storage, config, reports, tooling outside B/C | provider catalog surfaces; throughput/log-throughput surfaces | d5f94dd3 + 16eb6830 pushed/integrated; stopped |
| B | Provider model catalog | `a32b107df02d9b5b503b8d545a4fb24b7754e997` | `worker/provider-catalog-20260908` | worker worktree | provider catalog, Super model pickers, Kiro/Copilot/Gemini/OpenAI catalog consumers and tests | throughput/log-throughput; unrelated refactor | checkpoint pushed/integrated; worker stopped |
| C | Throughput observability | `08eafe5eb22cf67cee9c301b186445154eb28cc0` | `worker/throughput-observability-20260908` | worker worktree | generation timing, token usage, throughput state, log TUI, history, throughput docs/tests | provider catalog; unrelated refactor | 88294302 pushed/integrated; stopped |
| D | Read-only review | `a32b107df02d9b5b503b8d545a4fb24b7754e997` | none | none | exact checkpoint review only | all writes | completed exact-checkpoint reviews; no repository-wide claim |

Worker contract: each worker commits only reviewed paths to its worker branch, pushes that branch,
verifies its remote SHA, and reports focused tests/cleanup. Workers do not merge or push the
integration branch. Heavy full-workspace, Mojo, and campaign integration gates remain serial here.

## Invariants

- Preserve `previous_response_id`, `x-codex-turn-state`, and session-scoped `session_id` ownership.
- Rotate only before commitment; never rotate a committed stream or convert transport failure to quota.
- Preserve upstream status, headers, body, stream payload, CLI/API compatibility, and error semantics.
- Keep authentication, authorization, tenant isolation, secret redaction, accounting, persistence,
  Mojo ABI, and dependency direction unchanged.
- Keep request hot paths bounded and non-blocking; runtime notices remain in resolved log output.

## Evidence

- Baseline checks passed formatting, Markdown, workspace all-target/all-feature validation, Node, safe Rust shards, auto-rotate coverage, duplicate-budget guard, and the initial runtime graph inventory; graph results remain discovery evidence, not completeness proof.
- Exact green anchor: qualification repair `de244d63`, CI `34224751563`; the current campaign branch is the successor of that source and keeps release hold/write scope on the campaign branch.
- B1 provider catalog checkpoint `61f33d58` was independently reviewed and its complexity repair is restored by `f913ba60`; C1 throughput, A1 temp-file naming, Wave 4 root cleanup, Wave 5 policy, and Wave 6 CLI/session-store checkpoints remain retained but held by `208aa412`.
- Historical CI `34184199873` exposed provider-picker complexity and an unrelated Windows timeout; the exact current CI `34287156704` also exposed the broker test size budget and platform fixture gaps, all repaired or explicitly recorded in the current wave.
- Current local validation includes exact B0-036 security review `3712bbbe`, test extraction review `848e68fe`, provider/macOS review `f913ba60`, broker/app/runtime smoke, boundary guards, Node `264/1`, size `32/32`, Mojo share/non-regression, and workspace Clippy; full cross-platform/live-provider coverage is not claimed.
- All worker/reviewer SHA, test, public-API, security, compatibility, and cleanup evidence remains in `audit.csv`, retained branches, and campaign logs.

## Prior verified batches

- B1 provider catalog authority remains at the `a32b107d` baseline; its later ordering/degraded-status checkpoint is held for a later qualifying batch. The previously reviewed complexity-only repair is restored in the current CI repair checkpoint.
- C1 generation-throughput observability, A1 shared temp-file naming, Wave 4 root-import cleanup, Wave 5 runtime-policy cleanup, and Wave 6 CLI/session-store cleanup retain their exact worker/reviewer evidence but are currently held by commit `208aa412` to admit the critical broker repair.
- Waves 3–6 were executed with disjoint ownership and serial integration; their detailed checkpoints, tests, public-API decisions, and source evidence remain in `audit.csv`, retained worker branches, and campaign logs. No held checkpoint was deleted or treated as integrated without a qualifying source tree.

## Wave 7 manual successor execution

- Handoff completed: the legacy coordinator, controller, and worker trees were stopped gracefully; the protected manual session, user expose, Codebase Memory daemon, main worktree, release worktrees, and unrelated processes were not stopped.
- Current qualifying source tip: `240aead2` on `refactor/successor-integration-20260908`. Approved B0-036 commits `659cdc4e`, `18bd36a5`, and `f1f07236` were reviewed at exact worker SHA `3712bbbe`; the size repair `1a5a28d9` and CI/provider/macOS repair `240aead2` were independently reviewed with `NO_FINDING`.
- B0-036 validation passed broker 33, app `runtime_broker` 59, `runtime_doctor` 21, URL-boundary regressions, runtime smoke, size guard 32/32, runtime hot-path and manifest guards, auth/secret/application/crate boundary guards, workspace Clippy, fmt, and diff checks.
- Commit `208aa412` mechanically holds 24 lower-priority behavior-file checkpoints so the enforced churn range can admit the critical broker repair. Their remote branches, reviews, logs, and source evidence remain retained; no main or release worktree was changed.
- Held source domains are recorded as `IN_PROGRESS` in `audit.csv`, including CLI, session-store, runtime policy, runtime cookies, CI guards, provider picker repairs, throughput observability, temp-file naming, update notice, and test-import cleanups. B0-036 is the current `REFACTORED` source row.
- The official share report on the integrated source reports Rust `348,191`, Mojo `26,420`, total `374,611`, and share `7.052649281521365%`; the release floor and Mojo non-regression pass, the project target remains unmet, and the historical waiver is expired. The frozen fixture is aligned to that measured report.
- Current ledger counts are 64 unique B0 domains: 23 `UNREVIEWED`, 14 `IN_PROGRESS`, 2 `REFACTORED`, and 25 `KEEP_WITH_REASON`. B0-036 is closed only after exact-SHA review and focused validation; B0-031 remains in progress for its safe-metadata repair.
- Fresh CI run `34287156704` on `9484d4ae` failed static size budget before repair, Sonar complexity for the held picker source, and unrelated macOS/Windows tests; the first two are repaired locally and the platform failures are recorded in the retained evidence. A fresh run is required for `240aead2`.
- Final B0-036 writer/reviewer worktrees and targets were cleaned after report harvest; campaign logs and remote checkpoints remain retained.

## Known checkpoints and blockers

- Historical candidate commits were inspected through `afe20dfd`; no campaign documentation or active PR was present.
- Existing prunable worktree records and other campaign branches are outside this campaign and remain untouched.
- Active user Prodex/Codex processes are protected. No live-provider or credential-bearing tests are authorized.

## Cleanup and next action

- The successor integration worktree is retained. Clean stale audit worktrees from completed
  exact-base audits were removed after checking clean status and process cwd; active audit/review
  worktrees and their target caches remain protected until final reports are harvested. Historical
  campaign, release, and issue64 worktrees with dirty or unknown ownership remain protected; two
  prunable records remain untouched. Remote worker branches/checkpoints and campaign logs remain
  as evidence. No campaign-created coordinator/server/watcher remains active, and no scratch
  fixture or credential-bearing test was created.
- Next action at this boundary: push and monitor the current successor head, harvest exact final
  reports, clean only eligible campaign artifacts, and admit the next implementation wave only
  after target/resource checks. Do not claim campaign completion while any `UNREVIEWED`,
  `IN_PROGRESS`, or `BLOCKED` row remains.
