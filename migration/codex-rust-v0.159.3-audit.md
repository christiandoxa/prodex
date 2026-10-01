# Codex rust-v0.159.3 compatibility audit

## Disposition

CODEX_0_159_3_COMPATIBILITY=PRODEX_BASELINE_UPDATED_SECURITY_SETUP_IDENTITY_BOUNDARY_TRACKED.

Codex 0.159.3 is a focused TUI patch over 0.159.2. Eligible local sessions
authenticated with ChatGPT may fetch and display an optional account-security
setup reminder. The exact tagged source trees differ in 24 files with 833
insertions and 5 deletions. Apart from the workspace version bump, the patch is
confined to the TUI reminder lifecycle and its tests.

Prodex already routes model traffic through a dedicated model provider while
leaving `chatgpt_base_url` Codex-owned for account/read bootstrap traffic. That
boundary is required by 0.159.3: the reminder fetch verifies that the connected
app-server authentication matches the saved local ChatGPT login, then calls
`/wham/security-setup` through the configured ChatGPT bootstrap origin. Prodex
therefore needs no runtime routing change, but now guards that upstream identity
and origin contract explicitly.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.159.3
- Annotated tag object: 8e46774a94a745ffdf676bd7a8aa36466bbd4f99
- Peeled commit: 01fc69f4026735edfdf6789820549727a4867b11
- Release: 0.159.3, non-prerelease, published 2026-09-30T22:57:34Z
- Previous compatibility target: rust-v0.159.2
- Exact tagged-tree comparison: 24 changed files, 833 additions, and 5 deletions.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.159.2 source archive | da1fb76975416fe137f174dd3fd6c7b778ace11629f0607451474bfd78d98d83 |
| rust-v0.159.3 source archive | 7a13f939c8b5ea4e5fb1434f0b4a5ea3f4c85b22a4782818cb28a43ed0deac58 |
| codex-x86_64-unknown-linux-musl.tar.gz | b48ca1b2d6b1bf42b944e02c3d937c898e24651916684cdc35fdedf31b291bcb |
| extracted Codex binary | 8bf204b36a2f6dd0dab73aa2f639892e67ef9ac8befccb4a05b1496ebf25c479 |
| codex-app-server-x86_64-unknown-linux-musl.tar.gz | 1d782ebee6bc65a211bf4ed87702c037d5481e145fe76cb1b07e41cf612338b8 |
| extracted app-server binary | fe701715e787ae766b546303460b2efda556088aa99b55b569cf997d5805d775 |

The 0.159.3 source archive is 16,577,987 bytes. The official Linux musl CLI
asset is 108,495,560 bytes and its extracted binary is 287,086,056 bytes. The
official app-server asset is 84,154,313 bytes and its extracted binary is
221,902,840 bytes. Both asset hashes match GitHub release metadata. The CLI
reports `codex-cli 0.159.3`.

## Exact compatibility decisions

| Surface | 0.159.3 result | Prodex decision |
| --- | --- | --- |
| Security-setup eligibility | Local OpenAI-provider sessions may prefetch a reminder only when the saved credential is ChatGPT auth, the account is not FedRAMP, and the connected app-server reports the same ChatGPT auth mode and credential. Remote workspaces skip the fetch. | Keep account/read bootstrap traffic Codex-owned and add an upstream guard for the saved-login/app-server identity match. Do not rotate this request through Prodex model-account selection. |
| Security-setup request | The TUI uses a route-aware client with redirects disabled, a three-second timeout, the configured `chatgpt_base_url`, saved ChatGPT auth headers, and the Codex login user agent to GET `/wham/security-setup`. It rechecks identity after the fetch. | Preserve the existing dedicated model-provider proxy design so `chatgpt_base_url` remains outside the loopback model proxy. Track the exact upstream request/origin markers. |
| Reminder action URL | A notice is accepted only with bounded/control-free copy and an HTTPS action URL on `chatgpt.com` with no username, password, or custom port. | Treat this validation as upstream-owned UI security behavior and guard the host restriction. |
| Existing provider/protocol contracts | Responses HTTP/WebSocket routing, GPT-6.1 Sol model metadata, model-provider auth, quota/error classes, project trust, sessions, compaction, Windows background-process handling, and app-server lifecycle markers remain present. | Keep the 0.159.2 Prodex routing, context-budget, auth, quota, trust, process, and protocol boundaries unchanged. |

## Verification

- Downloaded and SHA-256 verified exact 0.159.2 and 0.159.3 tagged source
  archives and compared their extracted trees: 24 changed files, 833 additions,
  and 5 deletions.
- Replayed all 511 existing critical-file markers from the 0.159.2 Prodex
  baseline against the exact 0.159.3 source tree: zero missing markers.
- Verified the official 0.159.3 Linux musl CLI and app-server assets against the
  SHA-256 digests published in GitHub release metadata.
- Verified the extracted CLI reports `codex-cli 0.159.3`.
- Ran an isolated official 0.159.3 app-server `initialize` handshake with
  `capabilities.experimentalApi=true` under a synthetic HOME/CODEX_HOME. It
  returned `platformFamily=unix`, `platformOs=linux`, and the expected synthetic
  Codex home without credentials or a model turn.
- Confirmed the Prodex runtime-launch regression suite keeps account/read
  `chatgpt_base_url` Codex-owned while routing only model traffic through the
  governed local model provider.
- Added explicit critical-file and semantic guards for the 0.159.3
  security-setup identity, origin, redirect, timeout, and action-host contract.

## Prodex 0.434.3

Prodex 0.434.3 targets Codex rust-v0.159.3. It preserves the existing model
transport and profile-rotation behavior while keeping the new account-security
reminder on Codex's authenticated ChatGPT bootstrap path.
