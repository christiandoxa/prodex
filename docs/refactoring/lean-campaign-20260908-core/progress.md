# Lean Refactoring Campaign 20260908 Core

## Objective

Reduce proven duplication, dead forwarding, and scattered semantic ownership while preserving
public compatibility, security boundaries, runtime affinity, storage behavior, and provider
transport semantics. This is an implementation campaign, not a line-count exercise.

## Baseline and branch

- `BASE_SHA`: `afe20dfd9f813eb217f78b63de517c7e8e52e8b7`
- Baseline ancestry: `origin/main` (`ff7976858c048269d73e970356aa41f4d0b4cafb`) is an ancestor.
- Branch: `refactor/lean-campaign-20260908-core`
- Baseline source: historical `origin/refactor/428-integration-20260907`; no active PR or descendant campaign was found.
- Main write authorization: not granted. Release hold: true. Checkpoint push scope: campaign branch only.
- Protected main-worktree WIP: `.playwright-mcp/`; `crates/prodex-app/src/runtime_broker/registry/direct.rs`.

## Status

`IN_PROGRESS`; B's catalog, C's throughput, and A's general refactor checkpoints are integrated.
Full CI run `34190204961` passed on exact `5c42efee`, including Sonar, Real Mojo/parity, macOS,
all Windows shards, runtime stress, app shards, and relevant guards. B2 completed an audit-only
catalog consumer review with a concrete KEEP_WITH_REASON; A2 produced no result and remains
UNREVIEWED. Final reviewer D returned NO_FINDING on source tree `34800090`; the last ledger anchor
before this note is `f9eb135d`. The wider domain audit and final integration qualification remain
open; CI `34193580014` is pending on that anchor. The exact HEAD produced by this note is reported
after commit/remote verification rather than written into the same commit.

## Parallel ownership ledger

Integration branch: `refactor/parallel-integration-20260908`.

| Worker | Workstream | Base SHA | Branch | Worktree | Owns | Excludes | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A | General lean refactor | `34972926449f8201c925893ae5be3d8e5bb6976c` | `worker/refactor-general-20260908` | worker worktree | CLI, orchestration, profile/auth, session, runtime support, gateway, storage, config, reports, tooling outside B/C | provider catalog surfaces; throughput/log-throughput surfaces | d5f94dd3 + 16eb6830 pushed/integrated; stopped |
| B | Provider model catalog | `a32b107df02d9b5b503b8d545a4fb24b7754e997` | `worker/provider-catalog-20260908` | worker worktree | provider catalog, Super model pickers, Kiro/Copilot/Gemini/OpenAI catalog consumers and tests | throughput/log-throughput; unrelated refactor | checkpoint pushed/integrated; worker stopped |
| C | Throughput observability | `08eafe5eb22cf67cee9c301b186445154eb28cc0` | `worker/throughput-observability-20260908` | worker worktree | generation timing, token usage, throughput state, log TUI, history, throughput docs/tests | provider catalog; unrelated refactor | 88294302 pushed/integrated; stopped |
| D | Read-only review | `a32b107df02d9b5b503b8d545a4fb24b7754e997` | none | none | exact checkpoint review only | all writes | complete; A/B/C checkpoints no findings |

Worker contract: each worker commits only reviewed paths to its worker branch, pushes that branch,
verifies its remote SHA, and reports focused tests/cleanup. Workers do not merge or push the
integration branch. Heavy full-workspace, Mojo, and final integration gates remain serial here.

## Invariants

- Preserve `previous_response_id`, `x-codex-turn-state`, and session-scoped `session_id` ownership.
- Rotate only before commitment; never rotate a committed stream or convert transport failure to quota.
- Preserve upstream status, headers, body, stream payload, CLI/API compatibility, and error semantics.
- Keep authentication, authorization, tenant isolation, secret redaction, accounting, persistence,
  Mojo ABI, and dependency direction unchanged.
- Keep request hot paths bounded and non-blocking; runtime notices remain in resolved log output.

## Evidence

- Baseline command: `rtk npm run test -- --jobs 1 --timings` — exit `0`.
- The runner passed formatting, Markdown lint, workspace all-target/all-feature check, Node tests,
  safe Rust shards, and auto-rotate integration coverage. Exact per-shard test counts are pending
  a compact list/count capture; no failure was observed.
- Baseline size measurement: 2,379 Rust files; repository size-guard classification reports
  1,817 production files and 512,038 production lines, 32 near-limit files, no violations.
- Dependency duplicate guard: 21/21 budgeted families, exit `0`.
- Codebase Memory index: 59,103 nodes and 324,553 edges; five partial files and one ignored example
  asset are recorded as coverage limits. Graph results remain discovery evidence, not completeness proof.
- B1 checkpoint: local and remote `b8064149b615b60c56fd38302cdcf113a8c33890`.
- Predecessor catalog PR: `#70`, base `refactor/428-integration-20260907`; it is closed as
  superseded, while its branch and commits remain available as pre-orchestration evidence.
- Draft integration PR: `#71`, base `refactor/lean-campaign-20260908-core`, head
  `refactor/parallel-integration-20260908`; its body will be updated with reviewed checkpoints
  and final gate evidence.
- B1 repair CI run `34184199873` completed with failure on the exact SHA `a32b107d`: Sonar
  reported `rust:S3776` cognitive complexity `27` at
  `crates/prodex-app/src/runtime_tools/sub_agent_catalog.rs:66`, and the Windows prodex-app
  shard reported `544 passed; 1 failed; 2 ignored`, with a websocket pre-commit continuation
  timeout. `compat-replay-gate` was successful and optional-tools freshness was skipped by CI.
- Read-only reviewer D found no P0. The initial P1 Sonar issue was fixed; the unchanged Windows
  timeout was not reproduced in the later full run. The two initial P2 catalog findings—dynamic-
  before-canonical OpenAI ordering and discarded main-picker degraded status—were fixed in B's
  reviewed checkpoint.
- Parallel integration baseline: local and remote `a32b107df02d9b5b503b8d545a4fb24b7754e997` on
  `refactor/parallel-integration-20260908`; first-wave worker branches all start at this SHA.
- B repair checkpoint: local and remote `61f33d58f47f2ebef0b5ad104a14aaf407676382`; reviewer D
  found no P0-P3 issue. It was integrated without conflict as `0cec9519` on the integration branch.
- Integration focused tests on `0cec9519`: `super_main_prompt` 8 passed and `sub_agent_catalog`
  14 passed, both serial; `cargo fmt --check` and `git diff --check` passed.
- The official Mojo share report on the integrated source tree produced Rust `348,118` LOC, Mojo
  `26,420` LOC, total `374,538`, share `7.054023890766758%`, release-floor and non-regression
  PASS. The frozen Node expectation was repaired in `90a5b8b3`; local `npm run test:node` passed
  264 with 1 skipped. Integration CI run `34186438047` was superseded after its completed guard
  jobs passed.
- Wave 2 base: A and C worktrees were fast-forwarded cleanly to `08eafe5eb22cf67cee9c301b186445154eb28cc0`;
  neither had prior WIP or a prior checkpoint. C completed its bounded throughput batch; A
  completed the bounded temp-file owner consolidation after the CI hold.
- CI run `34186705160` failed on exact SHA `710f2401`: Sonar passed its source scan, but Real
  Mojo/parity failed compilation with `clippy::items_after_test_module` at
  `crates/prodex-app/src/app_commands/super_main_catalog.rs:630`; A/C were then paused. The test
  module was moved after production items in `071b8cbc`, with local strict clippy and focused
  tests passing. CI run `34187490415` was superseded after its completed guards passed. Full run
  `34187555697` passed on exact SHA `34972926449f8201c925893ae5be3d8e5bb6976c`, including the
  prior failing Windows remaining-library shard; the earlier websocket timeout was not reproduced.
- C worker checkpoint: local and remote `882943024a663e20ab0c27c470c6578a242c3ce7`; reviewer D
  reported NO_FINDING. It was integrated without conflict as `679f53f3`; final-tree focused tests
  passed for log throughput (19 combined), TUI (13), log integration (5), and runtime-proxy
  response forwarding (21 plus one zero-test auxiliary target).
- A worker checkpoints: local and remote `d5f94dd3be2dd9ae31a03832c19a0927920041c4` and
  `16eb68309204cfe11447131aeb5220030d7cfc19`; reviewer D reported NO_FINDING. They were integrated
  without conflict as `2b0bb24d` and `f9d8a05c`; final-tree tests passed for runtime-store (16),
  update-notice (11), and core (12).
- Current integration tree contains both worker streams through `f9d8a05c` plus guard repair
  `32daf41b`; local static guards and focused tests pass. Full CI `34190204961` passed on exact
  `5c42efee`, the audit-only wave checkpoint before the latest ledger updates. The final clean
  integration SHA is `f9eb135d8f76a6decf8205424ada205cf332b693`.
- Wave 3 ownership was disjoint: B2 owned only the gateway/dashboard catalog consumers in B1-007;
  A2 owned one general-domain candidate outside provider catalog and throughput files. B2 completed
  its audit-only review; A2 stopped before producing symbol-level evidence.
- B2 evidence: gateway focused tests passed `503`, with `17` ignored; dashboard focused tests passed
  `20`. The public metadata/availability contract is retained separately from picker catalog
  planning, with the concrete KEEP_WITH_REASON recorded in `audit.csv`.
- Storage cleanup checkpoint: before cleanup the host was `68%` used / `145G` available; after
  cleanup it was `62%` used / `173G` available. Removed exact campaign-owned predecessor and
  completed worker/B2 worktrees after clean/remote/process verification, reclaiming approximately
  `28G`. Main `target/` (`5.7G`) and integration `target/` (`21G`) are retained as protected or
  reusable caches; A2's clean handoff worktree is retained without a build target.
- Final local gate evidence: `npm run ci -- --no-tests --jobs 1` exited `0` after release hygiene,
  metadata, workspace all-target/all-feature check, strict clippy, and all non-test guards passed.
  The preceding `npm run ci -- --serial --jobs 1` logged PASS for test-fast and test-serial but
  did not expose a final parent exit line; it is not used as the sole completion claim.
- Final reviewer D checked exact `7dc21ac55ac80a68f1133fe36fde387bda9bbcdd` and reported
  NO_FINDING; the later `fe28e4b8` and `f9eb135d` changes are ledger-only and preserve that source
  review. This note is the final ledger update before handoff.

## Batch B1: provider catalog authority

Contract: `ProviderId` plus existing bounded local catalog files -> canonical provider choices plus
case-insensitive dynamic additions, `Custom` entry, or model-scoped effort fallback; malformed or
missing expected sources -> canonical choices plus bounded `Degraded` status; reads only, no network,
writes, profile mutation, or runtime transport changes; canonical order precedes first-seen dynamic
order and `Custom` remains last.

- Root cause: `super_prompt::configured_sub_agent_models_from_paths` only loaded Copilot/Kiro and
  returned an empty vector for every read/parse failure, while `super_main_catalog` separately loaded
  only OpenAI `models_cache.json`. A broken Kiro snapshot therefore looked like a complete Luna/auto catalog.
- Refactor: `runtime_tools::sub_agent_catalog::effective_provider_model_catalog` now owns bounded
  source assembly for OpenAI, Copilot, Gemini, Kiro, DeepSeek, and Local. `prodex-provider-core`
  remains the single pure choice/identity/reasoning merge owner. Main and sub-agent picker callers
  consume the same app-owned source authority.
- Degraded state is generic and non-secret; source errors are not rendered. A stale/missing profile
  does not consume the usable-catalog bound, so a later healthy Kiro profile remains visible.
- Focused tests: sub-agent catalog `14/14`; main prompt `7/7`; provider-core catalog `24/24` real
  tests plus one zero-test auxiliary binary; Gemini `9/9`, DeepSeek `4/4`, Local `5/5`, Kiro
  `14/14`, Copilot `12/12`. Dashboard `21/21` and gateway `503 passed, 17 ignored` were also
  reproduced while evaluating the next consumer batch; those consumers are not part of B1.
- Static guards after B1: application boundary, crate boundary, size, secret boundary, Mojo share,
  Mojo authority/no-fallback, and runtime test manifest all passed. All-feature workspace clippy
  passed on the final B1 source tree.
- Measurement with the repository size guard: baseline `512,038` production Rust lines across
  `1,817` files; B1 `512,426` lines across the same file count. The increase is bounded loader and
  regression coverage; semantic ownership reduced by deleting the old picker-only loader and its
  test-only forwarding helpers. No dependency or runtime transport change.

## Batch C1: retained generation throughput observability

Contract: authoritative output tokens plus positive provider-reported monotonic generation duration
-> unchanged `output_tokens * 1000 / generation_ms`; active display -> `gen N t/s`; completed
display -> `last gen N t/s` plus coarse monotonic age when available; historical age remains
unknown; no TTFT, request timing, log receipt timing, content, credentials, or network discovery.

- Worker checkpoint: local and remote `882943024a663e20ab0c27c470c6578a242c3ce7`; reviewer D
  reported no finding; integrated without conflict as `679f53f3`.
- Tests: log-throughput 13, state 6, TUI 13, log integration 5, runtime-proxy response-forwarding
  21 plus one zero-test auxiliary target; all serial and passing. Formula cases cover 50, 40, 50,
  and 80 t/s. `cargo fmt --check`, `git diff --check`, and the full static guard parallel runner
  passed.

## Batch A1: shared atomic temp-file naming

Contract: existing target basename/fallback, PID/timestamp/sequence/`.tmp` naming, separate atomic
sequences, private permissions, durability sync, replacement order, cleanup, errors, and platform
branches remain unchanged while duplicate naming logic uses the existing core owner.

- Worker checkpoints: `d5f94dd3be2dd9ae31a03832c19a0927920041c4` and
  `16eb68309204cfe11447131aeb5220030d7cfc19`, both remote verified; reviewer D reported no
  finding. Integrated as `2b0bb24d` and `f9d8a05c` without conflict.
- Tests: app runtime-store 16, update-notice 11, core 12; owning-crate all-target clippy,
  `cargo fmt --check`, and `git diff --check` passed.

## Wave 3 ownership

| Worker | Base SHA | Branch | Worktree | Owns | Excludes | Status |
| --- | --- | --- | --- | --- | --- | --- |
| A2 | `3bfb043c03f6ea926bda6b7d6e8cb8790080a283` | `worker/refactor-general-b2-20260908` | worker worktree | one evidence-backed general-domain refactor outside B/C | all provider catalog and throughput/log-throughput paths | stopped; clean, no result; UNREVIEWED |
| B2 | `3bfb043c03f6ea926bda6b7d6e8cb8790080a283` | `worker/provider-catalog-b2-20260908` | worker worktree | `gateway_kiro_model_catalog_json_from_paths`, dashboard models catalog consumer, and focused tests | throughput/log-throughput; A2 general refactor | audit complete; KEEP_WITH_REASON |
| D | current integration SHA | none | none | read-only exact-checkpoint review | all writes | resume for Wave 3 checkpoints |

## Known checkpoints and blockers

- Historical candidate commits were inspected through `afe20dfd`; no campaign documentation or active PR was present.
- Existing prunable worktree records and other campaign branches are outside this campaign and remain untouched.
- Active user Prodex/Codex processes are protected. No live-provider or credential-bearing tests are authorized.

## Cleanup and next action

- Campaign worktree is owned by this campaign. Its `target/` build cache is retained while validation continues.
- No campaign-created server or watcher remains active. All agents are stopped. Removed worktrees:
  predecessor lean campaign, A/B/C completed worker worktrees, and B2 audit-only worktree; four
  stale prunable records were removed after verifying their directories were absent. Retained:
  integration worktree/cache for final gates, A2 clean unreviewed handoff worktree, and remote
  worker branches/checkpoints. No scratch fixture, server, watcher, credential, or raw log was
  created for commit.
- Next action: perform the next symbol-level general-domain audit from the current integration
  SHA `f9eb135d`, or hand off the remaining UNREVIEWED inventory without claiming campaign completion.
