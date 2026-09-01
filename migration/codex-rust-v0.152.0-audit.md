# Codex `rust-v0.152.0` compatibility audit

## Provenance

- Repository: `https://github.com/openai/codex`
- Previous tag object: `d8673cb68e349c208659b986697773d3145dbb14`
- Previous peeled commit: `78c290807ce710180111df227df3b7a4fe845452`
- Target tag object: `7f6bee13af649d0da23ac0c2bf5c83f571fcd611`
- Target peeled commit: `316795b3cf2a45e90d121d9f46499d4658b2645c`
- Target release: `0.152.0`, published `2026-09-01T01:58:32Z`, non-prerelease
- npm package: `@openai/codex@0.152.0`
- npm integrity: `sha512-Vx0tg/J5SbxYYGJazTtL/XySK9Dlqc5KW1MZM71NMwVci/4F1ap+FfSKPFTlrICEtOTuq3KNcWSdv9oMGdPuRw==`

The comparison is the exact `rust-v0.151.0..rust-v0.152.0` diff from the
coordinator-owned mirror at `/tmp/prodex-upstream-codex-0152`. The mirror is
temporary audit state, not a repository dependency.

## Prodex boundary decisions

| Stable change | Prodex impact | Decision |
| --- | --- | --- |
| `codex-rs/exec/src/exec_events.rs` and `event_processor_with_jsonl_output.rs` | No source diff between the two tags. The `codex exec --json` lifecycle remains `thread.started`, `turn.started`, `item.*`, `turn.completed`, `turn.failed`, and `error`. | Keep the existing ping completion contract. A non-empty completed `agent_message` is sufficient; exact response text is not checked. |
| App-server auth-recovery notifications and additive request/response fields | Prodex forwards app-server JSON frames through its existing envelope/lifecycle boundary; it does not deserialize the complete upstream payload into a competing model. | No runtime change. Preserve unknown additive fields and existing affinity validation. |
| `GetAccountRateLimitsResponse` account/banner fields | Additive upstream usage metadata. Prodex quota parsing already preserves unknown JSON fields at its provider boundary and does not infer new model buckets. | No quota or Luna Reserve inference. |
| Package-style MCP names (`:`, `@`, `/`, `.`) | Codex accepts a wider upstream name grammar. Prodex does not own Codex MCP server-name parsing. | No local normalization or rejection change. |
| MCP elicitation, per-tool `output_token_limit`, and refreshed tool caches | Codex owns MCP client behavior and tool-output budgeting. Prodex's exposed MCP server has its own already-tested contract. | Do not duplicate or reinterpret upstream client features. |
| `thread/shellCommand.timeoutMs`, sleep/clock classification, Guardian compaction, and model-catalog-driven instructions | Additive or upstream-owned behavior outside Prodex's normal ping and profile-routing boundaries. | Keep passthrough, approval, session, and child-process ownership unchanged. |

## Version ownership

The tested default is updated to `0.152.0` in the owning Codex compatibility
module, npm manifest/lockfile, and Unix/Windows installers. Historical
`0.150.1` and `0.151.0` references remain where they describe prior release
evidence or compatibility fixtures.

## Verification

The exact upstream source audit verified that the JSONL files used by
`prodex ping openai` did not change. Prodex focused tests cover the all-profile
inventory, profile pinning, failure continuation, typed quota/503/protocol/
spawn results, valid non-`PONG` responses, and aggregate JSON output. Existing
app-server, quota, MCP, continuation, and security tests remain the regression
boundary for additive 0.152.0 behavior.

Source references:

- <https://github.com/openai/codex/tree/rust-v0.152.0/codex-rs/exec>
- <https://github.com/openai/codex/blob/rust-v0.152.0/codex-rs/exec/src/exec_events.rs>
- <https://github.com/openai/codex/blob/rust-v0.152.0/codex-rs/exec/src/event_processor_with_jsonl_output.rs>
- <https://github.com/openai/codex/blob/rust-v0.152.0/codex-rs/core/src/client.rs>
- <https://github.com/openai/codex/blob/rust-v0.152.0/codex-rs/app-server-protocol/src/protocol/common.rs>
- <https://github.com/openai/codex/blob/rust-v0.152.0/codex-rs/config/src/mcp_types.rs>
