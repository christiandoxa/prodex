# Codex rust-v0.159.2 compatibility audit

## Disposition

CODEX_0_159_2_COMPATIBILITY=PRODEX_BASELINE_UPDATED_WINDOWS_BACKGROUND_PROCESS_CONTRACT_TRACKED.

Codex 0.159.2 is a focused patch over 0.159.1. The release suppresses flashing
console windows on Windows when Codex launches background processes and sandboxed
commands. The exact tagged source trees differ in 47 files with 387 insertions and
76 deletions. The change centralizes console-free background command construction
and updates process-launch call sites; Prodex's Responses transport, GPT-6.1 Sol
catalog/context contract, provider authentication, quota/error classification,
project trust, session persistence, and app-server protocol remain compatible.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.159.2
- Annotated tag object: 8b9fa496bbf2c47aebd62e85a080b9a522a455b5
- Peeled commit: ff6aec96948b70d94983af2641a6b67c94faeff5
- Release: 0.159.2, non-prerelease, published 2026-09-29T23:57:16Z
- Previous compatibility target: rust-v0.159.1
- Exact tagged-tree comparison: 47 changed files, 387 additions, and 76 deletions.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.159.1 source archive | dbc5cad4ea2d7cf52996b05c275313e5f84418b3f3afb6a620aa491eadb6e6a1 |
| rust-v0.159.2 source archive | da1fb76975416fe137f174dd3fd6c7b778ace11629f0607451474bfd78d98d83 |
| codex-x86_64-unknown-linux-musl.tar.gz | 26586b0d246d41a799b0ef8ee1add370f0fb0721b3709340f28db612381616ea |
| extracted Codex binary | 1748767b230ebfc3d4ab7e4e254920d0c0ad9691fd8c11f190e7d44511a4a92e |
| codex-app-server-x86_64-unknown-linux-musl.tar.gz | 17d91bf7962fa3d171f077e0b50cb89156f5e7109eedcf9f82803e5d86fba7fc |
| extracted app-server binary | 17c7ddab590ccbd94a3ef75ac78a5d1db52ab0d5f36659cd3dce1b5d8012e2b5 |

The 0.159.2 source archive is 16,570,416 bytes. The official Linux musl CLI
asset is 108,518,339 bytes and its extracted binary is 286,754,152 bytes. The
official app-server asset is 84,112,746 bytes and its extracted binary is
221,865,976 bytes. Both asset hashes match GitHub release metadata. The CLI
reports `codex-cli 0.159.2`.

## Exact compatibility decisions

| Surface | 0.159.2 result | Prodex decision |
| --- | --- | --- |
| Windows background commands | Adds `codex_utils_process::background_command`, which applies `CREATE_NO_WINDOW` on Windows and ordinary `Command::new` elsewhere. | Track the helper as a critical upstream process-launch contract; no Prodex reimplementation is required. |
| Redirected shell-tool children | `StdioPolicy::RedirectForShellTool` now uses the background helper while `StdioPolicy::Inherit` keeps normal command creation. | Preserve upstream interactive-vs-background distinction and add a semantic marker for the call site. |
| Windows Job Object launches | Suspended contained/background launches combine `CREATE_NO_WINDOW` with `CREATE_SUSPENDED`, with a console-free fallback when containment cannot be established. | Keep the behavior upstream-owned; Prodex does not override Codex Job Object creation flags. |
| Existing provider/protocol contracts | GPT-6.1 Sol model metadata, Responses HTTP/WebSocket routing, auth/cache identity, quota/error classes, project trust, compaction, MCP naming, and app-server lifecycle markers remain present. | Keep the 0.159.1 Prodex routing, context-budget, auth, quota, trust, and protocol boundaries unchanged. |

## Verification

- Downloaded and SHA-256 verified exact 0.159.1 and 0.159.2 tagged source
  archives and compared their extracted trees: 47 changed files, 387 additions,
  and 76 deletions.
- Verified the official 0.159.2 Linux musl CLI and app-server assets against the
  SHA-256 digests published in GitHub release metadata.
- Verified the extracted CLI reports `codex-cli 0.159.2`.
- Replayed the complete existing critical-file and semantic marker set against
  the exact 0.159.2 source tree with zero misses, then added explicit markers for
  the new background-process helper and redirected-shell call site.
- Ran an isolated official 0.159.2 app-server `initialize` handshake with
  `capabilities.experimentalApi=true` under a synthetic HOME/CODEX_HOME. It
  returned `platformFamily=unix`, `platformOs=linux`, and the expected synthetic
  Codex home without credentials or a model turn.
- The offline upstream baseline guard and self-test pass with the 0.159.2 patch
  record and process-launch markers.

## Prodex 0.434.1

Prodex 0.434.1 targets Codex rust-v0.159.2. It retains the GPT-6.1 Sol
872,000-token maximum-context accommodation added for 0.159.1 and additionally
tracks the 0.159.2 Windows background-process console-suppression contract.
