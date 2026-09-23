# Codex rust-v0.156.1 compatibility audit

## Disposition

`CODEX_0_156_1_COMPATIBILITY=PRODEX_CONTEXT_UPDATE_AND_EXISTING_WORKSPACE_HOTFIX_REQUIRED`.

Codex 0.156.1 is a focused hotfix over 0.156.0. The tagged source changes 21
files with 461 insertions and 77 deletions. Of Prodex's 50 pinned critical
compatibility files, only `codex-rs/models-manager/models.json` changes.

The release makes GPT-6 Sol and GPT-6 Luna visible in the Codex picker. The
0.156.0 workspace-routing HTTPS boundary is byte-identical in 0.156.1, so the
Prodex 0.431.1 launch repair remains required: Prodex must keep
`chatgpt_base_url` Codex-owned/HTTPS and route model traffic through its
authenticated local provider instead of replacing the ChatGPT bootstrap URL
with an HTTP loopback URL.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: `rust-v0.156.1`
- Annotated tag object: `81e8e29b2956dfe9b092c63953a9ed282781e77c`
- Peeled commit: `b412ff32c417f855c2b2d1581b77058eed87c84b`
- Release: `0.156.1`, non-prerelease, published `2026-09-23T02:41:36Z`
- Previous compatibility target: `rust-v0.156.0`
- Exact tagged-tree comparison: 21 changed files, 461 additions, 77 deletions.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.156.0 source archive | `a2e008b0985566188b3662f83a1c3320e791103d9e8e0a547430bc6141a4f580` |
| rust-v0.156.1 source archive | `cd1e3d063dcf5e029815b7bb178c9017221ddd12bd49b879b14c1f4b09219a02` |
| codex-x86_64-unknown-linux-musl.tar.gz | `aff46539a83aff86e3c62c592bce2c50d95391f9df289afaf03a50c01d14533d` |
| extracted codex binary | `0b2e9301d6100dddda3b9d5c80ebaeaa3a2f1962388f2f36f6b96a9f08b1f33f` |

The official Linux musl asset is 107,380,357 bytes and the extracted binary is
284,479,848 bytes.

The workspace-routing source SHA-256 is
`2d2e1badd55ac8805ac344f8b921ac10d7f6bc0b8b9679108ddb3bb288f09c92`
for both 0.156.0 and 0.156.1.

## Exact compatibility decisions

| Surface | 0.156.1 result | Prodex decision |
| --- | --- | --- |
| GPT-6 Sol | Visible picker model, 272k base / 872k maximum context, default Medium, efforts Low/Medium/High/XHigh/Max/Ultra. | Preserve upstream model metadata; recognize 872k when deriving runtime and Smart Context budgets. |
| GPT-6 Luna | Visible picker model, 272k base / 872k maximum context, default Medium, efforts Low/Medium/High/XHigh/Max. | Preserve upstream model metadata; recognize 872k when deriving runtime and Smart Context budgets. Do not infer the legacy GPT-5.6 Luna reserve mapping. |
| GPT-5.6 Sol | Receives an upstream upgrade target to GPT-6 Sol. | Preserve the additive upgrade metadata opaquely. |
| Rate-limit switch prompt | Upstream TUI recommends GPT-6 Luna. | No Prodex prompt rewrite. Preserve Codex-owned UI behavior. |
| Workspace routing | Source is byte-identical to 0.156.0. `account/read` still requires a credential-free HTTPS backend origin. | Retain the 0.431.1 launch fix that keeps `chatgpt_base_url` HTTPS/Codex-owned. |
| Responses / WebSocket / compaction / auth | No compatibility-critical source changes versus 0.156.0. | Keep the 0.156.0 routing, affinity, pre-commit retry, and authentication contracts unchanged. |

## Verification

- Downloaded and SHA-256 verified exact 0.156.0 and 0.156.1 tagged source
  archives.
- Compared the exact tagged trees. Only one of the 50 pinned Prodex critical
  files changed: `codex-rs/models-manager/models.json`.
- Replayed all critical-file string assertions against 0.156.1; every current
  required marker remains present.
- Verified the official Linux musl asset checksum and `codex-cli 0.156.1`.
- An isolated official app-server `model/list` returned both `gpt-6-sol`
  and `gpt-6-luna` as visible models with the expected reasoning effort sets.
- Repeated the synthetic ChatGPT `initialize -> account/read` reproduction
  against the official 0.156.1 binary using the former HTTP loopback
  `chatgpt_base_url`. It still returns JSON-RPC `-32603` with
  `workspace backend must use an HTTPS origin without credentials`.
- The workspace-routing source file has the exact same SHA-256 in 0.156.0 and
  0.156.1.
- Prodex runtime tests cover the GPT-6 872k context-window behavior and retain
  the 0.431.1 launch projection tests for both Rust-oracle and release-pinned
  Mojo production paths.

The audit used only synthetic auth material and isolated temporary homes. It
did not access or modify the user's real Codex credentials.

## Prodex 0.431.1

Prodex 0.431.1 targets Codex `rust-v0.156.1`. The release combines the
workspace-routing bootstrap fix required by both 0.156.0 and 0.156.1 with
runtime awareness for the two newly visible GPT-6 models.

## Prodex 0.431.3 Super trust behavior

A real pseudo-TTY launch of the published Prodex 0.431.2 binary with official
Codex 0.156.1 confirmed that folder trust was already bypassed, but Codex still
showed one startup warning because Super supplied
`--dangerously-bypass-hook-trust`.

The 0.431.3 candidate keeps the workspace project at `trust_level="trusted"`
but changes hook trust to Codex's native trust-state flow. Before starting the
TUI, Prodex calls `hooks/list`, writes each untrusted or modified hook's
`currentHash` into `hooks.state.<key>.trusted_hash` through
`config/batchWrite`, and verifies a second `hooks/list` has no remaining
review-required hooks. The global bypass flag is retained only as a compatibility
fallback when an older Codex reports the hook-trust RPC methods as unavailable.

The same real interactive smoke with the candidate reached the Codex 0.156.1
TUI in YOLO mode with zero startup warnings, no folder-trust prompt, no
hook-review prompt, three Ponytail hooks trusted by hash, and no global
hook-bypass flag on either Codex child process. Codebase Memory MCP and
Playwright MCP were running, Caveman awareness was present, RTK resolved to
0.49.0 in the user's login-shell PATH, and Ponytail 4.10.0 remained enabled.
Presidio's default-off interactive opt-in flow is unchanged.
