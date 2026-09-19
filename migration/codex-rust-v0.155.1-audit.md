# Codex rust-v0.155.1 compatibility audit

## Disposition

CODEX_0_155_1_COMPATIBILITY=NO_RUNTIME_CHANGE_REQUIRED.

Codex 0.155.1 is a narrow patch over 0.155.0. The only behavior change is the
new-local-TUI app-server reasoning-summary default: absent explicit config, the
TUI now sends model_reasoning_summary = "none" instead of "detailed".
Prodex provider-bridge launches already pin model_reasoning_summary="none", so
this upstream correction aligns with existing Prodex behavior. No Responses,
compaction, affinity, header, model-cache, provider-auth, MCP, or app-server
protocol change is required.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.155.1
- Annotated tag object: 4e21628f9ec9ee656650cd2b62ef92225725b5ac
- Peeled commit: be2951ea34f0d295ed0becf97079f92fa5f6950e
- Release: 0.155.1, non-prerelease, published 2026-09-18T20:03:04Z
- Previous compatibility target: rust-v0.155.0
- Exact source diff: 3 changed files, 23 additions, 6 deletions, no binary entries.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.155.0 source archive | 8bb3b5a5693492926b53c51ee2e68822b057d712b2ca1a0ae67878557fd6a2ea |
| rust-v0.155.1 source archive | b9e18d40d322586913e94d6747f3f934922c4f5130eb5a349ba019c57b83dad8 |
| codex-x86_64-unknown-linux-musl.tar.gz | a0ef8b2debc3bf747e07b1a039354de31300ac0dcc2276498ba281470b5d9115 |
| extracted codex binary | 0753dfe1d8b87a52436deb13eb1c549661ef4c84fee2c5aa688385eebeccb761 |

The official Linux musl asset is 101,581,479 bytes.

## Exact compatibility decisions

| Surface | 0.155.1 result | Prodex decision |
| --- | --- | --- |
| TUI reasoning-summary default | new_thread_reasoning_overrides now defaults model_reasoning_summary to none, not detailed; explicit settings remain respected. | Keep provider-bridge launch overrides explicit at model_reasoning_summary="none". Do not depend on the upstream implicit default. |
| Responses transport and headers | No source changes from 0.155.0. | Keep current metadata/header transparency, auth replacement, and pre-commit rotation behavior. |
| Remote compaction v2 | No source changes from 0.155.0. | Keep ordinary /responses compaction-trigger passthrough and hard affinity; retain legacy /responses/compact compatibility for older clients. |
| Model catalog/cache identity | No source changes from 0.155.0. | Keep model cache identity upstream-owned and provider/account scoped. |
| App-server protocol | No protocol/schema source changes from 0.155.0. | Preserve unknown/additive fields and current capability-probed behavior; no broker recreation. |
| MCP/provider auth | No source changes from 0.155.0. | Keep current name normalization, auth isolation, and provider boundary behavior. |

## Verification

- Exact 0.155.0 and 0.155.1 tagged source archives were downloaded and hashed.
- The tagged-tree diff contains only codex-rs/Cargo.toml,
  codex-rs/tui/src/app_server_session.rs, and
  codex-rs/tui/src/app_server_session/reasoning_defaults_tests.rs.
- The official Linux musl asset digest matches the GitHub release digest.
- The extracted official binary reports codex-cli 0.155.1.
- codex app-server --help still exposes daemon, generate-ts, and
  generate-json-schema.
- An isolated stdio app-server initialize with capabilities.experimentalApi=true
  returned a valid 0.155.1 result from a synthetic home without credentials or
  a model turn.
- Prodex CLI regressions assert provider-bridge launch args explicitly contain
  model_reasoning_summary="none".
- The offline upstream baseline self-test and normal guard must pass with
  rust-v0.155.1 as the tested release.

The audit did not modify the user's installed Codex binary, credentials, or live
orchestrator.

## Prodex 0.430 preparation

This audit supersedes the 0.155.0 compatibility pin for the Prodex 0.430
release train. The release remains gated by the repository's broader feature,
Mojo-ownership, upgrade, artifact, and release-hygiene requirements.
