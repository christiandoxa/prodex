# Codex rust-v0.162.0 compatibility audit

## Disposition and scope

Prodex 0.437.0 qualifies the existing OpenAI Responses HTTP/WebSocket, compact,
affinity, quota, and app-server boundaries against Codex `rust-v0.162.0`.
This updates the audited upstream reference and compatibility guard. It does not
install or replace Codex, raise Prodex's capability-based minimum, change release
versions or changelog files, or move Codex-owned policy into Prodex.

The review covers changed runtime boundary owners named in the release request.
Runtime-launch/Codex configuration behavior and binary smoke evidence remain
coordinator-owned and are not claimed here. No live provider or model traffic was
used.

## Provenance

- Repository: https://github.com/openai/codex
- Stable release: https://github.com/openai/codex/releases/tag/rust-v0.162.0
- Published: 2026-10-08T18:55:59Z; not a draft or prerelease.
- Annotated tag object: `1f3f93473394b620b35580859b7e6864f7a9f948`.
- Peeled commit: `c1382380de69521303b416720a52f42d51af6248`.
- Previous target: `rust-v0.161.0` at `979011409de0a60b52f179721948e65531d26144`.
- Exact extracted source trees contain 1,658 changed files: 1,400 changed
  common files, 188 additions, and 70 removals. The GitHub compare API is capped
  at 300 files and was not used as a full-tree inventory.
- Archive hashes, file counts, and replay totals are in
  `migration/codex-rust-v0.162.0-source-evidence.json`.

| Source archive | SHA-256 |
| --- | --- |
| `source-01610.tar.gz` | `4b20acb969588c0694f97851aaf63228eb206fa052b168681503f63c2e4efac2` |
| `source-01620.tar.gz` | `89c5f8139486a9ce723681e977109e1512b039b7e010e2ee9a3a1a6a24952369` |

## Reviewed boundary changes

### Responses failure and WebSocket retry guidance

`codex-api/src/sse/responses_error.rs` now reads JSON `headers` from
`response.failed`, converts them through the shared HTTP header parser, and
retains validated `Retry-After` advice for overload, rate-limit, and generic
retryable errors. Invalid advice still falls back to the existing bounded
message-based handling. `Duration::try_from_secs_f64` rejects non-finite delay
values.

`codex-api/src/endpoint/responses_websocket.rs` keeps retry metadata from both
nested WebSocket error headers and top-level event headers. Upgrade failures
continue to preserve HTTP response headers. Prodex passes these upstream errors
through and keeps its no-midstream-rotate and hard-affinity rules.

### Custom provider capabilities and moved owner

Codex 0.162 adds serializable custom-provider overrides for `external_web_access`
and `remote_compaction`. OpenAI and Azure Responses providers default to remote
compaction V2; unrelated providers default to `Unsupported`, and explicit
provider overrides win.

The `is_azure_responses_provider` marker moved from
`model-provider/src/provider.rs` to the new `model-provider/src/capabilities.rs`.
The baseline follows that owner instead of dropping the guard. Prodex does not
recompute or override Codex provider capability decisions.

### Namespace exposure and incremental input identity

The provider-level `namespace_tools` gate is removed. `core/src/tools/spec_plan.rs`
now keeps namespace merging and applies direct-only namespace overrides and strict
third-party Code Mode policy through explicit tool exposure logic. The baseline
tracks that source owner so the old hidden capability gate cannot silently return.

Responses Lite incremental context now tracks top-level tool definitions and base
instructions as stable world-state items. `core/src/client.rs` prepends the shared
prefix to request input and uses the Responses Lite or raw Responses API tool
shape as appropriate. Base-instruction identity is recognized through internal
content metadata; Prodex does not rewrite these Codex-owned prompt items.

### Partial-answer phase

Responses stream parsing retains `phase: "partial_answer"` on output item start
and completion. Rollout conversation normalization places both `partial_answer`
and `final_answer` in the final answer channel while preserving the upstream
phase distinction. Prodex forwards stream events without synthesizing or replaying
partial output.

### Remote MCP Windows environment restoration

The exact 0.162 source restores `SYSTEMROOT`, `TEMP`, and `TMP` beside the
standard environment allowlist when explicit remote environment variables are
requested. The existing `UNREQUESTED_SECRET` regression remains, so the allowlist
does not become a full environment copy. This supersedes the 0.161.0 documented
limitation; the historical 0.160.1 and 0.161.0 audit records remain unchanged.

### App-server lineage and environment additions

App-server v2 turn requests and responses carry parent/root turn lineage and
turn environment requests. Environment registration accepts exact required skill
catalog names and converts them into scoped executor configuration before model
inference. Core input attribution retains the initiating agent path and causal
root across recovery, sleep, and compaction.

## Narrow qualification constraints

- Codex Cyber/Daybreak eligibility remains provider-owned. The exact source still
  filters automatic access-program routing to `OPENAI_PROVIDER_ID`; Prodex's
  governed provider identity does not bypass that requirement.
- This audit qualifies transport/header preservation, source contracts, and local
  compatibility guards. It does not claim successful model turns, entitlement,
  provider availability, or cross-platform binary behavior.
- Existing Prodex hard affinity, no-midstream rotation, upstream error passthrough,
  and 0.436.1 capacity fixes remain outside this source-only change.

## Verification run by this worker

- `node scripts/compat/check-upstream-baseline.mjs --self-test` — pass. Includes
  negative guard cases for failed/WebSocket retry headers, custom capability
  defaults, namespace gate removal, partial answers, remote MCP Windows variables,
  app-server lineage, and environment skills.
- `node scripts/compat/check-upstream-baseline.mjs` — pass with zero errors and
  zero warnings.
- `node scripts/compat/check-upstream-baseline.mjs --source <exact rust-v0.162.0 extracted source directory>` — pass. Replayed every `critical_files[].required_contains` and `semantic_checks[].file_contains_all` entry: 77 critical files, 86 semantic groups, 1,238 markers, zero missing files or markers. The source hashes and counts are recorded in `migration/codex-rust-v0.162.0-source-evidence.json`.
- `node scripts/compat/capture-replay-fixture-tests.mjs` — pass, 5 fixture/self-check cases.
- `git diff --check` — pass.

Runtime-launch, official-binary, app-server, and Prodex transport tests are not
claimed until run by their owning worker.
