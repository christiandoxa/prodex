# Codex `rust-v0.154.0` compatibility audit

## Disposition

`CODEX_0_154_COMPATIBILITY=NO_PRODUCTION_CHANGE_REQUIRED`.

The official stable tag was resolved, the exact Linux binary was tested in an
isolated home, and the 0.153.4-to-0.154.0 app-server/session/queue boundary
smokes passed. Prodex already preserves the additive 0.154 fields and already
sends the experimental capability required by the queue API. No production
compatibility patch or live-orchestrator upgrade is justified.

## Provenance

- Repository: <https://github.com/openai/codex>
- Tag: `rust-v0.154.0`
- Annotated tag object: `36eab01061df3cde5f95ec20a526777b430091ba`
- Peeled commit: `6b9826e3aa83b1a5947db50f4332cb9c65f1b340`
- Release: `0.154.0`, non-prerelease, published `2026-09-09T22:35:38Z`
- Release metadata: <https://github.com/openai/codex/releases/tag/rust-v0.154.0>
- Previous compatibility tag: `rust-v0.153.4`, tag object
  `042fb41b7c813ac7999105e886b2b7aa715b5081`, peeled commit
  `3d2ee51ca2d5db578f328aa75e20aa22c0197c9a`
- GitHub compare: diverged range, 249 commits, merge base
  `2c79ee6dacb6deccb7e19ac5acffb3e379bbe895`.

Exact source archive hashes:

| Artifact | SHA-256 |
| --- | --- |
| `rust-v0.153.4` source archive | `74d988c0e154aad2b8d0cca4e950fc97fe2a29ff5ebe3b0070cce6d949c9a307` |
| `rust-v0.154.0` source archive | `1c4cdc3b87ba290b5d110425b4f6ff21663e236580bc760d1e149bd2d9f9519f` |
| `codex-x86_64-unknown-linux-musl.tar.gz` | `d7e18b2597ae8f242f5f31ee9e90deef48dbc9edd634d9868fb6435d08c07f02` |
| extracted `codex` binary | `3188814c35471432d4123203e0eb38e5bddc60226e3d7ddf0e59e649ea140022` |

The release asset is 98,981,886 bytes and has upstream digest
`sha256:d7e18b2597ae8f242f5f31ee9e90deef48dbc9edd634d9868fb6435d08c07f02`.

## Actual baseline and safety boundary

- Live orchestrator: Codex CLI `0.153.4`, resolved through
  `/home/doxa/.local/share/upkit/node/active/bin/codex` to the existing
  installation under the active overlay; its app-server process identity was
  left untouched.
- `codex --version`: `codex-cli 0.153.4`.
- 0.154 binary: isolated official extracted binary, `codex-cli 0.154.0`, under
  the campaign compatibility scratch directory with a separate `HOME`,
  `CODEX_HOME`, XDG directories, temp directory, and process identity.
- No installer, updater, package manager, credentials, provider turn, or active
  orchestrator executable was mutated.

## Delta and compatibility decisions

| Surface | Exact result | Decision |
| --- | --- | --- |
| Session/thread start | 0.154 adds `environments`, `originator`, and `daybreakEnabled` fields while preserving thread/session/cwd/status/direct-input fields. | Prodex parses the response as JSON values, validates the existing identity/cwd/status contract, and ignores additive fields safely. |
| Resume/fork/lifecycle | 0.154 rechecks state after configuration work and reports closing-thread conditions; daemon/PID lifecycle work is upstream-owned. | Prodex uses the existing `thread/read` authority check and its own fail-closed process identity; no duplicate lifecycle implementation. |
| Prompt queue | `thread/queue_processor.rs` and queued-submission protocol definitions are unchanged from 0.153.4 to 0.154.0. Both versions require `experimentalApi` for `thread/queue/add`. | Existing Prodex capability request and exact `queuedSubmission` validation remain correct. Queue acceptance remains `QUEUED`, never `APPLIED`. |
| Quota/rate limits | 0.154 adds `supportsLunaReserve`, `excludeResetCreditDetails`, `ordinaryUsageAllowed`, and `normalModelSlug`. | Existing Prodex quota deserialization, explicit Reserve/model checks, unknown-field retention, and no-inference policy already cover these fields. |
| Output/rollout | No Prodex-owned Responses/SSE/WebSocket contract was changed by the inspected app-server delta; 0.154 additive thread fields are not treated as new event semantics. | Keep opaque passthrough and bounded schema validation. Unknown events do not become equivalent to known events. |
| Plugins/MCP/tools | 0.154 refreshes plugin/MCP capability behavior and adds optional user-verification/worktree APIs. | Prodex does not assume optional capabilities and does not enable experimental upstream behavior. |
| Installation/updater | 0.154 adds managed updater/daemon behavior, especially on Windows. | Compatibility testing used the official binary directly; Prodex does not self-update the live orchestrator or replace Codex binaries. |

## Required smokes and validation

Commands and outcomes:

1. `git ls-remote https://github.com/openai/codex.git refs/tags/rust-v0.154.0 refs/tags/rust-v0.154.0^{}` — exact annotated tag and peeled commit matched the provenance above.
2. Official isolated binary `codex-cli 0.154.0 --version` — pass.
3. Isolated `codex app-server --listen stdio://` initialize and `thread/start` against both 0.153.4 and 0.154.0 — pass; both returned matching identity/cwd/status contracts. No model turn was sent.
4. Isolated `initialize` with `capabilities.experimentalApi=true`, `thread/start`, `thread/queue/add`, `thread/queue/list`, and `thread/queue/delete` against both versions — pass. Both returned valid queued-submission IDs and matching synthetic input; neither emitted `turn/started` or `turn/completed`. Queue list/delete immediately returned no actionable item in both runs; this is retained as queue behavior, not application evidence.
5. `node scripts/compat/check-upstream-baseline.mjs` — pass after advancing the pinned baseline metadata to 0.154.0.
6. `CARGO_TARGET_DIR=<isolated> node scripts/ci/runtime-test-manifest-guard.mjs` — pass: 55 manifest cases, 61 broad shard filters, 29 stress hints, 61 workflow filters, 854 enumerated Cargo tests.
7. `CARGO_TARGET_DIR=<isolated> cargo test --locked -q -p prodex-app --lib app_server_broker_compat -- --test-threads=1` — pass, 8/8.
8. `CARGO_TARGET_DIR=<isolated> cargo test --locked -q -p prodex-app --lib session_prompt_write -- --test-threads=1` — pass, 55/55.

The broad Cargo enumeration is recorded as enumeration, not as executed test
count. No live credential-bearing or cost-bearing turn was used.

## Final decision

The 0.154 delta is compatible with the current Prodex boundary. The only
campaign changes are the auditable compatibility target metadata, guard pin,
documentation correction for newly exposed quota fields, and this audit. The
live orchestrator remains pinned to 0.153.4 until the campaign terminates.
