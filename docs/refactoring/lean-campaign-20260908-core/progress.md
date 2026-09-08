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

`IN_PROGRESS`; first-wave implementation is held after the B catalog checkpoint while the
repaired integration CI completes. A and C remain stopped with clean worktrees; B's checkpoint is
integrated and D reviewed it with no finding. The wider domain audit remains open.

## Parallel ownership ledger

Integration branch: `refactor/parallel-integration-20260908`.

| Worker | Workstream | Base SHA | Branch | Worktree | Owns | Excludes | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A | General lean refactor | `a32b107df02d9b5b503b8d545a4fb24b7754e997` | `worker/refactor-general-20260908` | worker worktree | CLI, orchestration, profile/auth, session, runtime support, gateway, storage, config, reports, tooling outside B/C | provider catalog surfaces; throughput/log-throughput surfaces | stopped; clean, no checkpoint |
| B | Provider model catalog | `a32b107df02d9b5b503b8d545a4fb24b7754e997` | `worker/provider-catalog-20260908` | worker worktree | provider catalog, Super model pickers, Kiro/Copilot/Gemini/OpenAI catalog consumers and tests | throughput/log-throughput; unrelated refactor | checkpoint pushed/integrated; worker stopped |
| C | Throughput observability | `a32b107df02d9b5b503b8d545a4fb24b7754e997` | `worker/throughput-observability-20260908` | worker worktree | generation timing, token usage, throughput state, log TUI, history, throughput docs/tests | provider catalog; unrelated refactor | stopped; clean, no checkpoint |
| D | Read-only review | `a32b107df02d9b5b503b8d545a4fb24b7754e997` | none | none | exact checkpoint review only | all writes | complete; B checkpoint no finding |

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
- Predecessor catalog PR: `#70`, base `refactor/428-integration-20260907`; it remains the
  pre-orchestration checkpoint and is not the integration review surface.
- Draft integration PR: `#71`, base `refactor/428-integration-20260907`, head
  `refactor/parallel-integration-20260908`; its body will be updated with reviewed checkpoints
  and final gate evidence.
- B1 repair CI run `34184199873` completed with failure on the exact SHA `a32b107d`: Sonar
  reported `rust:S3776` cognitive complexity `27` at
  `crates/prodex-app/src/runtime_tools/sub_agent_catalog.rs:66`, and the Windows prodex-app
  shard reported `544 passed; 1 failed; 2 ignored`, with a websocket pre-commit continuation
  timeout. `compat-replay-gate` was successful and optional-tools freshness was skipped by CI.
- Read-only reviewer D found no P0, but marked both the Sonar issue and unchanged Windows timeout
  as P1 blockers. The two initial P2 catalog findings—dynamic-before-canonical OpenAI ordering and
  discarded main-picker degraded status—were fixed in B's reviewed checkpoint.
- Parallel integration baseline: local and remote `a32b107df02d9b5b503b8d545a4fb24b7754e997` on
  `refactor/parallel-integration-20260908`; first-wave worker branches all start at this SHA.
- B repair checkpoint: local and remote `61f33d58f47f2ebef0b5ad104a14aaf407676382`; reviewer D
  found no P0-P3 issue. It was integrated without conflict as `0cec9519` on the integration branch.
- Integration focused tests on `0cec9519`: `super_main_prompt` 8 passed and `sub_agent_catalog`
  14 passed, both serial; `cargo fmt --check` and `git diff --check` passed.
- The official Mojo share report on the integrated source tree produced Rust `348,118` LOC, Mojo
  `26,420` LOC, total `374,538`, share `7.054023890766758%`, release-floor and non-regression
  PASS. The frozen Node expectation was repaired in `90a5b8b3`; local `npm run test:node` passed
  264 with 1 skipped. Integration CI run `34186438047` targets exact SHA `90a5b8b3` and is pending.

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

## Known checkpoints and blockers

- Historical candidate commits were inspected through `afe20dfd`; no campaign documentation or active PR was present.
- Existing prunable worktree records and other campaign branches are outside this campaign and remain untouched.
- Active user Prodex/Codex processes are protected. No live-provider or credential-bearing tests are authorized.

## Cleanup and next action

- Campaign worktree is owned by this campaign. Its `target/` build cache is retained while validation continues.
- No campaign-created server or watcher remains active after baseline testing. A and C worktrees
  are clean and their agents are stopped; B's worktree is clean after its pushed checkpoint.
- Next action: finish CI run `34186438047`, inspect its corresponding Windows continuation
  coverage, classify the earlier timeout as reproduced or not reproduced, then resume A and C only
  if the shared gates are clean.
