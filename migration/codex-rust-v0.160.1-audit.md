# Codex rust-v0.160.1 compatibility audit

## Disposition

CODEX_0_160_1_COMPATIBILITY=PRODEX_BASELINE_UPDATED_NO_MODEL_TRANSPORT_CHANGE_REQUIRED.

Codex 0.160.1 is a focused patch over 0.160.0. The exact tagged source trees differ in only two files with 24 additions and 1 deletion. The only runtime behavior change preserves Windows executor bootstrap environment variables when remote stdio MCP launchers use explicitly configured remote environment variables.

Prodex model routing, Responses HTTP/WebSocket transport, auth, quota/failover, session continuity, app-server protocol, model/provider catalogs, and the 0.160.0 environment inheritance contracts are unchanged. Prodex therefore advances its audited reference to 0.160.1 without changing model transport.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.160.1
- Annotated tag object: c3e23d4c4385619ecec78408766e46b7fa7dd9ad
- Peeled commit: d27764b82f7118f674371e6d6e76271d9d606edb
- Release: 0.160.1, non-prerelease, published 2026-10-05T18:29:37Z
- Previous compatibility target: rust-v0.160.0
- Upstream patch: #51121, backporting Windows remote MCP environment preservation to 0.160
- Exact tagged-tree comparison: 2 changed files, 24 additions, 1 deletion

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.160.0 source archive | e9ba182239bb984f1d24a57c8b3abb3da127e0a108e5f17d7a9038688cf4369c |
| rust-v0.160.1 source archive | 5226394058e04c5404fe737b84fbede38ebf41d88a72eb8b27f3d1c6ea23373b |
| codex-x86_64-unknown-linux-musl.tar.gz | 9226581be592d18f7e7f740a352fdb63aa61e45e39f7eb9b09d3888c84bba33f |
| extracted Codex binary | f34a4d2301892ae96c90097786bfe5dc269f187b6f69faf42a7b357b8c081e35 |
| codex-app-server-x86_64-unknown-linux-musl.tar.gz | fa36836211386644a67345aeaeb4b1362322abf2771450e0684eb8ad1fb30525 |
| extracted app-server binary | 1a1918076fae6b48bcd73e858abf26ea2fd659835cbc3c267396c679737456bd |

The official CLI reports `codex-cli 0.160.1`.

## Exact compatibility decision

| Surface | 0.160.1 result | Prodex decision |
| --- | --- | --- |
| Remote stdio MCP environment | When explicit remote environment variables activate `include_only`, Codex now chains `SYSTEMROOT`, `TEMP`, and `TMP` after its default environment allowlist. This lets a Unix orchestrator preserve the Windows executor startup and temporary-directory environment. The upstream regression also proves unrelated remote secrets remain filtered. | Keep Prodex MCP launch behavior unchanged. Guard the exact upstream allowlist contract so future baseline updates cannot silently drop the Windows bootstrap variables or broaden the allowlist. |
| Other runtime surfaces | No source changes outside workspace version metadata and `rmcp-client/src/stdio_server_launcher.rs`. | Preserve all 0.160.0 Prodex transport, auth, quota/failover, session, provider, trust, and app-server behavior unchanged. |

## Verification

- Downloaded and SHA-256 verified exact 0.160.0 and 0.160.1 source archives.
- Compared extracted tagged trees: exactly 2 changed files, 24 additions, and 1 deletion.
- Replayed the complete Prodex compatibility baseline against rust-v0.160.1: 62 critical files and 65 semantic checks, with zero missing files or markers.
- `node scripts/compat/check-upstream-baseline.mjs --self-test` passes and the baseline guard reports zero errors and zero warnings.
- Verified the official Linux musl CLI and app-server assets against GitHub release digests.
- Verified the extracted CLI reports `codex-cli 0.160.1`.
- Ran an isolated official 0.160.1 app-server stdio `initialize` handshake with `capabilities.experimentalApi=true` under synthetic HOME/CODEX_HOME. It returned `platformFamily=unix`, `platformOs=linux`, the synthetic Codex home, and user agent `prodex-audit/0.160.1` without credentials or a model turn.
- Added explicit critical-file and semantic guards for the remote stdio MCP Windows bootstrap environment contract.

## Prodex 0.435.6

Prodex 0.435.6 targets Codex rust-v0.160.1 as its audited compatibility reference while continuing to accept capability-compatible Codex versions at runtime.
