# Codex `rust-v0.155.0` compatibility audit

## Disposition

`CODEX_0_155_COMPATIBILITY=NO_RUNTIME_CHANGE_REQUIRED`.

Prodex already carries the transport behavior needed by Codex 0.155 remote
compaction: a compaction request can travel through the ordinary Responses path
with a terminal `compaction_trigger` input item, preserve
`x-codex-turn-metadata`, and retain hard session/profile affinity. This audit
updates the pinned upstream owners and the exact 0.155 regression metadata. It
does not add a second compaction implementation, model-cache identity algorithm,
daemon, voice stack, or user-verification implementation.

## Provenance

- Repository: <https://github.com/openai/codex>
- Tag: `rust-v0.155.0`
- Annotated tag object: `799f378ee7f85c775dee82d9bc45cc2df8df18fb`
- Peeled commit: `f0a1b8f0849d90960bc406b848f32e5a129b0457`
- Release: `0.155.0`, non-prerelease, published `2026-09-17T23:14:43Z`
- Previous compatibility target: `rust-v0.154.0`
- Exact source diff: 1,330 changed files, 110,802 additions, 24,895 deletions,
  plus two binary entries.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| `rust-v0.154.0` source archive | `1c4cdc3b87ba290b5d110425b4f6ff21663e236580bc760d1e149bd2d9f9519f` |
| `rust-v0.155.0` source archive | `8bb3b5a5693492926b53c51ee2e68822b057d712b2ca1a0ae67878557fd6a2ea` |
| `codex-x86_64-unknown-linux-musl.tar.gz` | `e415cc3adb94ade16e8d44b4dd58a9201cc34b2ee51a5d6eddf2a3a00aecb6c0` |
| extracted `codex` binary | `660e159a49e823ac8e5986cb238f73158ce4b957d40d9292f8de90862644b501` |

The official Linux musl asset is 101,573,733 bytes.

## Exact compatibility decisions

| Surface | 0.155 result | Prodex decision |
| --- | --- | --- |
| Remote compaction | `compact_remote.rs`, `compact_remote_request.rs`, and the dedicated codex-api compact endpoint are removed. Remote compaction v2 appends `ResponseItem::CompactionTrigger {}` and streams through the normal `/responses` endpoint. | Preserve the ordinary Responses request and `x-codex-turn-metadata` opaquely. Keep existing legacy `/responses/compact` handling for older Codex clients, but no longer treat that route as the current upstream owner. |
| Compaction reasoning | `compact_remote_v2.rs` obtains effort through `reasoning_effort_for_request(..., RequestEffortUsage::Compaction)`. | Do not rewrite or infer compaction effort in Prodex. Keep model/request metadata transparent. |
| Compaction metadata | `CompactionTurnMetadata` now serializes `auto|manual`, `context_limit|user_requested|model_downshift|comp_hash_changed`, `responses|responses_compaction_v2`, and `pre_turn|mid_turn|standalone_turn`. | The Prodex compaction-v2 regression uses the exact `auto/context_limit/responses_compaction_v2/pre_turn` shape and verifies hard affinity plus metadata passthrough. |
| Model catalog cache | New `model-provider/src/models_identity.rs` scopes model caches to provider routing and effective auth/account identity. Cache entries carry an opaque identity alongside client version and ETag. | Treat model-cache identity as Codex-owned. Prodex profile isolation must not merge or synthesize cache identities across provider/account boundaries. |
| WebSocket ownership | Cached WebSocket state tracks `auth_owner_generation` and reconnects when ownership changes. | Keep Prodex account/profile rotation pre-commit and preserve hard affinity. Do not reuse a provider transport across an auth-owner change. |
| Model catalog | Bundled metadata adds `gpt-6-astra` and retains the GPT-5.6 Sol/Terra/Luna context-window contract. | Keep upstream catalog fields opaque and provider-scoped. Prodex public/provider catalogs remain independent compatibility surfaces. |
| App-server | 0.155 adds daemon recovery/update, verification cancellation, memory/status, and durable thread attachments. | These are optional upstream-owned APIs. Prodex preserves unknown/additive protocol fields and does not recreate the removed enterprise app-server broker stack. |
| Voice and Touch ID | 0.155 introduces experimental voice and native local verification surfaces. | No Prodex runtime implementation is required for the 0.430 compatibility baseline. Capability detection remains additive/fail-closed. |

## Verification

- Official asset digest matched the GitHub release digest.
- Official isolated binary reported `codex-cli 0.155.0`.
- `codex app-server --help` still exposes `daemon`,
  `generate-ts`, and `generate-json-schema`.
- An isolated stdio app-server `initialize` with
  `capabilities.experimentalApi=true` returned a valid 0.155 result without a
  model turn or credentials.
- `node scripts/compat/check-upstream-baseline.mjs --self-test` passes after
  moving the compatibility owners to the 0.155 source tree.
- `node scripts/compat/check-upstream-baseline.mjs` passes with
  `rust-v0.155.0` as the tested release.

The compatibility campaign uses source archives and an isolated official binary.
It does not modify the user's installed Codex binary, credentials, or live
orchestrator.

## Prodex 0.430 preparation

This audit is the Codex compatibility baseline intended for the future Prodex
0.430 release train. The minimum externally discovered Codex version remains
`0.153.2` for now; 0.155 compatibility does not require forcing existing users
to upgrade before the 0.430 release decision is made.
