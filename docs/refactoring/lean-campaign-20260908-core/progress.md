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

`IN_PROGRESS`; batch `B1-provider-catalog` is implementation-complete locally and awaits its
focused checkpoint. The wider domain audit remains open.

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
- Focused tests: sub-agent catalog `15/15`; main prompt `6/6`; provider-core catalog `24/24` real
  tests plus one zero-test auxiliary binary; Gemini `9/9`, DeepSeek `4/4`, Local `5/5`, Kiro `14/14`,
  Copilot `12/12`.

## Known checkpoints and blockers

- Historical candidate commits were inspected through `afe20dfd`; no campaign documentation or active PR was present.
- Existing prunable worktree records and other campaign branches are outside this campaign and remain untouched.
- Active user Prodex/Codex processes are protected. No live-provider or credential-bearing tests are authorized.

## Cleanup and next action

- Campaign worktree is owned by this campaign. Its `target/` build cache is retained while validation continues.
- No campaign-created server or watcher remains active after baseline testing.
- Next action: stage only the reviewed B0/B1 note and source paths, run cached-diff security/architecture
  guards, create and push the focused checkpoint to the campaign branch, verify its remote SHA, then
  continue the next domain only after synchronization.
