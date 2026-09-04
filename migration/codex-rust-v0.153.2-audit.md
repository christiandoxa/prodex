# Codex `rust-v0.153.2` compatibility audit

## Provenance

- Repository: <https://github.com/openai/codex>
- Previous tag object / peeled commit: `6bc50f104dcc0192e696cdeae721dfc19b507391` / `41e22fee981a63b3698df7ed36bad393cda24715`
- Intermediate tag object / peeled commit: `f5c2c463f1a92d62faf57da7516f72d4351afb6e` / `985641272869835d01d025ed2a218fbbce35fa9f`
- Target tag object / peeled commit: `79016fcca2c514d9c38643d8b7970a021e829b3b` / `657a993cbee87acf52d14b758ce49dbd46d1b8eb`
- Release timestamps from the upstream release feed: `.153.1` `2026-09-03T21:05:14Z`; `.153.2` `2026-09-03T23:54:50Z`

The exact tagged sources were downloaded from these immutable tag names:

| Tag | Archive SHA-256 |
| --- | --- |
| `rust-v0.153.0` | `165f53c44d4c58d0f0f812e582acd15eda97a35cd3764fe693dcef8693367c64` |
| `rust-v0.153.1` | `647b85605080bd79633a29d284c9ccd70aa81c404620d6feb8dce075cdd5dbcb` |
| `rust-v0.153.2` | `996929a8d112ad31d542de796ec4038981c6a29734dd54e8623db760436933a5` |

Reproduction commands:

```sh
curl -L https://github.com/openai/codex/archive/refs/tags/rust-v0.153.0.tar.gz -o rust-v0.153.0.tar.gz
curl -L https://github.com/openai/codex/archive/refs/tags/rust-v0.153.1.tar.gz -o rust-v0.153.1.tar.gz
curl -L https://github.com/openai/codex/archive/refs/tags/rust-v0.153.2.tar.gz -o rust-v0.153.2.tar.gz
diff -qr codex-rust-v0.153.0 codex-rust-v0.153.1
diff -qr codex-rust-v0.153.1 codex-rust-v0.153.2
diff -qr codex-rust-v0.153.0 codex-rust-v0.153.2
```

## Exact tagged-tree deltas

`rust-v0.153.0 → rust-v0.153.1` changes 11 paths: the Codex workspace version,
the model-size allowlist, Guardian production/tests, one MCP Guardian test,
the model catalog, and two TUI test helpers; it adds the 137-line
`guardian_v2_model_tests.rs`. The model catalog adds hidden GPT-6-Astra API
configuration, changes catalog priorities, and adds model-specific
`auto_review` metadata. The Guardian changes make computer-use review depend on
the model requirement and surface a declined elicitation as a tool error.

`rust-v0.153.1 → rust-v0.153.2` changes only `codex-rs/Cargo.toml` and
`codex-rs/models-manager/models.json`: the workspace version becomes `0.153.2`
and the GPT-6-Astra Fast-tier description changes from `1.5x` to `2x` speed.
The release explicitly says request behavior is unchanged.

## Prodex contract audit

| Area | Exact source result | Decision |
| --- | --- | --- |
| App-server lifecycle, schema, thread start/resume/fork/queue, existing-session queue | No Prodex-relevant upstream source change; the new app-server file is Guardian tests only. | Keep the existing session bridge and opaque additive-frame passthrough. |
| MCP lifecycle/event stream, prompt injection, output reads, process/thread identity, writer locks, command-server passthrough | No source diff in the owning upstream files. | No compatibility patch. Existing identity and queue proofs remain the authority. |
| Responses HTTP/SSE/WebSocket, raw usage, completion metadata, and TPS event boundaries | No source diff in Responses, compact, usage, or transport files. | Keep the measured-generation classifier and raw metadata passthrough unchanged. |
| Model catalog and model metadata | Additive hidden GPT-6-Astra configuration, priority changes, and Guardian metadata; `.153.2` changes display text only. | Keep upstream-owned metadata opaque; retain Prodex defaults. |
| Config, approvals, security, auth/provider transport | Guardian policy behavior changes only; no Prodex-owned config or provider source changes. | Preserve upstream launch/config arguments and do not duplicate Guardian policy. |
| Current time, compaction/image budgeting, remote-control/app-server transport, additive schema fields | No relevant source diff. | No compatibility repair. |

No Prodex compatibility implementation is required by either exact diff. The
active package, installer, baseline, and documentation pins advance together
to `@openai/codex@0.153.2` / `rust-v0.153.2`; the prior `.153.0` fixture and
historical audit remain historical evidence.

## Verification boundary

The release qualification reruns the upstream baseline guard, compatibility
replay, session/MCP identity proof, and final-candidate TPS proof after the
candidate SHA is fixed. No live credential-bearing upstream turn is used as a
fixture, and no Prodex default is changed for the new model catalog.

Source references:

- <https://github.com/openai/codex/releases/tag/rust-v0.153.1>
- <https://github.com/openai/codex/releases/tag/rust-v0.153.2>
- <https://github.com/openai/codex/compare/rust-v0.153.0...rust-v0.153.1>
- <https://github.com/openai/codex/compare/rust-v0.153.1...rust-v0.153.2>
