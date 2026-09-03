# Codex `rust-v0.153.0` compatibility audit

## Provenance

- Repository: <https://github.com/openai/codex>
- Previous annotated tag object: `3c6cfbab81e44218c729dc8c6b304cb760d1b8a1`
- Previous peeled commit: `5adb68a49933ae446bf11935662c83dba55a0804`
- Previous tree: `3e643e5ef6195a3881be9f8d6b394019786155ee`
- Target annotated tag object: `6bc50f104dcc0192e696cdeae721dfc19b507391`
- Target peeled commit: `41e22fee981a63b3698df7ed36bad393cda24715`
- Target tree: `f184d4ae6c14f9b591b8d5595c1f1bbb77cf18c1`
- Target release: `0.153.0`, published `2026-09-03T01:37:38Z`, non-prerelease
- npm package: `@openai/codex@0.153.0`
- Exact tree comparison: isolated mirror, `git diff refs/tags/rust-v0.152.1^{} refs/tags/rust-v0.153.0^{}`
- Topology: diverged; `git rev-list --left-right --count` reported `6 99`; neither tag is an ancestor of the other.
- Tree delta: `735` changed files, `36,379` additions, `8,616` deletions, including `147` additions, `576` modifications, `8` deletions, `4` renames, and `2` binary changes.

The release tag was rechecked with `git ls-remote`, the peeled commit and tree
were read from the exact tagged mirror, and the release page exposed the exact
publication timestamp. npm registry metadata independently reported these
published artifacts:

| Artifact | Version | Integrity |
| --- | --- | --- |
| `@openai/codex` | `0.153.0` | `sha512-k55kUZaclNi5ceUStSVuyW834ruA6AEdzTK7Xi3M1mOyXokUmq1sJLXm1RJ3XD2S7bRPeF1EXNsYB5Qxwus0mw==` |
| Linux x64 | `0.153.0-linux-x64` | `sha512-sFF+g7p1o6sZVL6PHd9JTYrP2j3xao3Qeu5AngcrD5brWrCc+w9ifBJCJuRSM728WfgjXML9PQPdHPaZSeHAEA==` |
| Linux arm64 | `0.153.0-linux-arm64` | `sha512-N2zLgmuslhMoC2RFC2aDkcCWk3iapZmBlSiESpjXFF+UMNxIpNxWJ5qQtQrCp5Md7kYbF4/EFy8RqJ8tL8UTeA==` |
| macOS x64 | `0.153.0-darwin-x64` | `sha512-DO5u+X/eL8pObrV0WDErOjS9DeuDvVuOL5oHR5y5Yklp0NGoGj3oOHsmaSXjGesiIiu3IX8l4pzS6dbeQXJAvQ==` |
| macOS arm64 | `0.153.0-darwin-arm64` | `sha512-C9F3HVEYekVJC3WhiGSztcxItWBwd7Y6hTWoDqa9Z9aLWsYywzqChABB59n7G5F5jCil9wxW3+u2L6uiVUydmw==` |
| Windows x64 | `0.153.0-win32-x64` | `sha512-eXvmitT1Hmr6d8n9r5HVjTuSgdHtxFSyXh0c3rRnaTcJA0ef8JnI5LY0TJci/QHYLNJGaEMBZ5R4dEdB2d87Lw==` |
| Windows arm64 | `0.153.0-win32-arm64` | `sha512-Ro+/2oAQnOObxuw3hpr+anv+j9hFmql/2iDxd7v0m46ctZy8x6ygW/Ud4f9CB1UXlRRsDhW+Tu9UAd5X9vTL1g==` |

## Compatibility decisions

Every release-tree behavior was assigned one of the following outcomes. The
large tree is primarily upstream implementation and test movement; Prodex
changes only its owned boundaries.

| Domain | Classification | Evidence and Prodex action |
| --- | --- | --- |
| Responses HTTP/SSE | `PRODEX_CHANGE_REQUIRED` | The 0.153.0 parser still uses `response.completed` and the existing Responses event names. Prodex now recognizes the legitimate generation deltas `response.reasoning_text.delta` and `response.mcp_call_arguments.delta` in its shared timing classifier. `response.created`, `response.in_progress`, and terminal `*.done` events remain excluded from generation start. |
| Responses WebSocket | `PRODEX_CHANGE_REQUIRED` | WebSocket inspection calls the same runtime-proxy classifier as SSE. Current event-shape coverage therefore receives the same two narrow generation boundaries without a second implementation. |
| Raw response usage | `UPSTREAM_OWNED_PASSTHROUGH` | PR #41980 (`e017e93aceafb2fe04bed1c926e448a5fb4f913d`) preserves the completed `usage` object in `ResponseUsageMetadata.metadata`. Prodex forwards unknown response fields and does not reinterpret raw metadata as timing. |
| Token usage persistence/replay | `UPSTREAM_OWNED_PASSTHROUGH` | PR #41912 (`5f79a92e3936274318d2122ae3244e5edd80dd1f`) persists response usage in Codex rollout history. Prodex does not own rollout storage; its proxy logs only observed Responses usage. |
| Measured throughput | `PRODEX_CHANGE_REQUIRED` | Actual output tokens remain divided by the monotonic interval from the first semantic output delta to `response.completed`. No bytes, characters, request latency, TTFT, or guessed ratio is used. Unary responses remain untimed without an authoritative generation duration. |
| App-server protocol/schema | `UPSTREAM_OWNED_PASSTHROUGH` | Thread model/reasoning metadata, `RawResponseCompletedNotification.usage_metadata.metadata`, and `request_user_input_async` are additive Codex contracts. Prodex's broker preserves validated JSON-RPC frames and unknown fields rather than reconstructing the upstream schema. |
| Session/thread/resume | `ALREADY_COMPATIBLE` | Thread/session/turn affinity remains Prodex-owned at the routing boundary; Codex owns rollout reconstruction and resume semantics. Existing continuation and resume fixtures remain applicable. |
| Rollout reconstruction/compression | `UPSTREAM_OWNED_PASSTHROUGH` | PR #42039 adds shared histories to upstream compression; PR #42135 permits forks from symlinked session roots. Prodex does not select compressed rollout internals or weaken path validation. |
| Guardian | `UPSTREAM_OWNED_PASSTHROUGH` | PRs #41870, #41879, #41919, #42065, #42144, #42147, and #42256 change upstream review evidence, metadata, analytics, or approval-mode scoring. Prodex preserves launch/config arguments and opaque metadata; it does not implement Guardian. |
| Approval/sandbox | `ALREADY_COMPATIBLE` | Prodex preserves approval and sandbox arguments at launch and does not override Codex Full Access or User approval behavior. |
| MCP event streams | `UPSTREAM_OWNED_PASSTHROUGH` | PRs #41892, #41899, and #41906 retain upstream MCP clients/subscriptions and add an event-stream manager. Prodex keeps MCP frames, account affinity, and secrets at existing boundaries. |
| MCP OAuth refresh | `UPSTREAM_OWNED_PASSTHROUGH` | PR #42128 coordinates upstream refresh. Prodex does not own refresh tokens or persist credential material. |
| Account-scoped app approvals | `UPSTREAM_OWNED_PASSTHROUGH` | PRs #42047, #42054, #42056, #42133, and #42134 scope upstream approvals to app links. Prodex profile affinity remains separate from app-account authorization. |
| Plugins/marketplaces | `UPSTREAM_OWNED_PASSTHROUGH` | PRs #41949, #41953, #42100, #42114, #42149, and #42150 change upstream reconciliation and source policy. Prodex forwards app-server/plugin frames and does not duplicate marketplace policy. |
| Config | `UPSTREAM_OWNED_PASSTHROUGH` | PR #41976 moves `disable_paste_burst` under `[tui]` with upstream fallback; PR #42101 adds `tui.auto_recap`. Prodex does not normalize or enable either setting. |
| Context management | `UPSTREAM_OWNED_PASSTHROUGH` | PR #42385 adds `features.context_management.experimental_mode`, disabled by default and gated upstream. Prodex does not inject or advertise this capability for custom/API-key providers. |
| Context/metadata | `UPSTREAM_OWNED_PASSTHROUGH` | PRs #41901, #41940, #42043, and related session changes remain Codex-owned. Prodex preserves opaque context metadata and existing bounded log fields. |
| Model metadata/catalog | `UPSTREAM_OWNED_PASSTHROUGH` | PR #42151 adds nullable thread `model` and `reasoningEffort`; Prodex does not treat either as profile identity or alter model selection. |
| Realtime | `UPSTREAM_OWNED_PASSTHROUGH` | PRs #41923 and #41924 add sideband endpoints and conversation history. Prodex keeps existing realtime bridge routes, affinity, and cleanup without forking Core semantics. |
| Network requirements | `UPSTREAM_OWNED_PASSTHROUGH` | PR #42173 adds header injections to upstream requirements. Prodex preserves safe metadata, keeps credential-like values out of logs, and retains its own outbound policy boundary. |
| Exec/JSONL | `ALREADY_COMPATIBLE` | The 0.153.0 tree retains `thread.started`, turn lifecycle, and completed structured JSONL semantics used by `prodex ping openai`; no Prodex completion rule change is needed. |
| Ping completion | `ALREADY_COMPATIBLE` | Ping remains healthy only after a completed structured response; exit zero alone is not success. Existing status/quota/protocol tests remain the boundary. |
| Release/runtime packaging | `PRODEX_CHANGE_REQUIRED` | Pin the canonical compatibility module to `0.153.0`, synchronize package aliases and lockfiles, and retain the six-platform artifact matrix. |
| Test-only upstream changes | `TEST_ONLY` | Upstream fixtures, schema bundles, platform build recipes, and deleted/redundant tests do not change a Prodex runtime contract. |
| Unrelated upstream domains | `NOT_RELEVANT` | Native voice, editor behavior, analytics, Bazel, formatting, and unrelated platform tooling are outside Prodex's owned boundary. |

## TPS protocol proof

The exact 0.153.0 `codex-api/src/sse/responses.rs` tree continues to parse
`response.reasoning_text.delta` and carries `response.mcp_call_arguments.delta`
as a current Responses wire event. The previous Prodex list omitted both. A
sanitized fixture at
`crates/prodex-runtime-proxy/tests/fixtures/codex-0.153.0-reasoning-first.sse`
contains only response lifecycle/event shape and synthetic IDs/content.

The producer path is covered by:

1. `RuntimeSseTapState` parsing split fixture chunks and emitting final usage
   with a positive `generation_ms`.
2. `RuntimeSseTapReader` consuming the same fixture and writing a final
   `token_usage` record with `output_tokens=7`, positive `generation_ms`, and
   positive `output_tokens_per_second`.
3. Existing log collector, duplicate live/disk, history-seed, sticky-idle,
   profile-rotation, flood, zero/missing usage, and TUI rendering tests.

The classifier intentionally does not start timing at `response.created`,
`response.in_progress`, response metadata, or terminal `*.done` events. It uses
`Instant` only. No trustworthy start or positive output usage still yields an
unknown rate. `responses_unary` still passes `generation_ms: None`, so it
remains `— t/s` unless Codex supplies an authoritative generation interval.

## Verification record

Initial machine forensic search found no current Prodex runtime log containing a
`token_usage`, `generation_ms`, or `output_tokens_per_second` record; therefore
no live credential-bearing turn was used as a fixture. The failure was instead
reproduced from the exact current wire event shape and the producer's missing
generation boundary, then covered through the runtime proxy and app-shaped SSE
tests. Live-provider and cost-bearing tests remain intentionally unrun.

The release qualification records the exact commands and results in the
campaign handoff. Historical release notes remain historical; current package,
installer, baseline, and documentation references are updated separately by
the release process.
