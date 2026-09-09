# Lean Refactoring Campaign 20260908 Core

## Objective

Reduce proven duplication, dead forwarding, and scattered semantic ownership while preserving
public compatibility, security boundaries, runtime affinity, storage behavior, and provider
transport semantics. This is an implementation campaign, not a line-count exercise.

## Historical baseline and branch

- `BASE_SHA`: `afe20dfd9f813eb217f78b63de517c7e8e52e8b7`
- Baseline ancestry: `origin/main` (`ff7976858c048269d73e970356aa41f4d0b4cafb`) is an ancestor.
- Branch: `refactor/lean-campaign-20260908-core`
- Baseline source: historical `origin/refactor/428-integration-20260907`; no active PR or descendant campaign was found.
- Main write authorization: not granted. Release hold: true. Checkpoint push scope: campaign branch only.
- Protected main-worktree WIP: `.playwright-mcp/`; `crates/prodex-app/src/runtime_broker/registry/direct.rs`.

## Status

The original B1 baseline remains documented below; current integration state is recorded in the
current-wave section below. The wider domain audit remains open.

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
- Draft PR: `#70`, base `refactor/428-integration-20260907`; CI is in progress/pending for B1,
  with `compat-replay-gate` successful and optional-tools freshness skipped by CI.

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

## Current wave

- Current source tip: `8cca48b0` on `refactor/successor-integration-20260908`; local and remote PR head match after the B0-031 integration.
- B0-036 security checkpoint `3712bbbe` received exact-SHA `NO_FINDING`; size repair `848e68fe` and CI/provider/macOS repair `f913ba60` also received `NO_FINDING` and are integrated.
- B0-031 safe metadata checkpoint `622ced1c` received exact-SHA `NO_FINDING` after runtime-Anthropic 11/15, app translation 45/45 in both modes, compat replay, smoke, guards, and Clippy; it is integrated as `e7de47db`.
- Current source preserves loopback broker admission, race-safe process identity, bounded legacy detection, safe Anthropic metadata forwarding, and 32/32 size guard; no thresholds or allowlists changed.
- B0-020, B0-032, B0-038, B0-039, B0-040, B0-041, and B0-043 are `KEEP_WITH_REASON`; B0-031 is `REFACTORED` after its exact-SHA review. Current B0 counts: 18 `UNREVIEWED`, 13 `IN_PROGRESS`, 3 `REFACTORED`, 30 `KEEP_WITH_REASON`.
- The current official share report is Rust `348,201`, Mojo `26,420`, total `374,621`, and `7.052461020604825%`; release floor and Mojo non-regression pass, project target remains unmet, and the waiver is expired.
- CI run `34295037209` is the current remote validation for `8cca48b0`; it failed only on the unrelated Windows continuation test (`547 passed, 1 failed, 2 ignored`). Full cross-platform, full-workspace, and live-provider coverage is not claimed locally.
- The successor remains the only campaign publication scope; main, release worktrees, user WIP, worker branches, and campaign logs remain protected.

## Known checkpoints and blockers

- Historical candidate commits were inspected through `afe20dfd`; no campaign documentation or active PR was present.
- Existing prunable worktree records and other campaign branches are outside this campaign and remain untouched.
- Active user Prodex/Codex processes are protected. No live-provider or credential-bearing tests are authorized.

## Cleanup and next action

- Campaign worktree is owned by this campaign. Its `target/` build cache is retained while validation continues.
- No campaign-created server or watcher remains active after baseline testing.
- Next action: continue the unresolved B0 audit queue, starting with storage/gateway/security domains; keep source writers and reviewers separate, preserve exact-base evidence, and admit only bounded qualifying batches.
