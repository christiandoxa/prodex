# Codex rust-v0.158.0 compatibility audit

## Disposition

CODEX_0_158_0_COMPATIBILITY=PRODEX_BASELINE_UPDATE_REQUIRED_NO_RUNTIME_BLOCKER.

Codex 0.158.0 is a broad release over 0.157.1. The exact tagged source trees
change 1,224 files with 52,257 insertions, 13,525 deletions, and 2 binary entries.
Prodex's pinned Responses, WebSocket, provider-auth, session-affinity, app-server
lifecycle, MCP naming, and workspace-routing contracts remain compatible after
two intentional upstream marker updates: the bundled Bedrock GPT-5.4 entry is
removed, and large tool-schema compaction now receives a caller-provided byte
budget while retaining a 5,000-byte default.

The release also extracts portable project-trust lookup. Codex 0.158 resolves
executor-normalized canonical cwd spellings before original spellings. Prodex
Super already projects both canonical and original workspace keys for direct
UUID resume and /resume, so the zero-prompt Super trust fix remains aligned with
upstream behavior.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.158.0
- Annotated tag object: 54e1bd264b4122fe9471ee7d54c4d021a76bb8ff
- Peeled commit: 064c6b8c737f5b41d171fdda80bd9ef10ad06eb3
- Release: 0.158.0, non-prerelease, published 2026-09-28T05:07:23Z
- Previous compatibility target: rust-v0.157.1
- Exact tagged-tree comparison: 1,224 changed files, 52,257 additions, 13,525
  deletions, and 2 binary entries.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.157.1 source archive | e3df5f38129e566723a9b762ef021019aedc830b06a57624d78a03a59b6a9d55 |
| rust-v0.158.0 source archive | e0c6f2492a1afad6ace9101b01a85deb58f81c52ee25af351d3b7478425a3558 |
| codex-x86_64-unknown-linux-musl.tar.gz | af9f5aa6e6662accf9d707cef0d9ca083880a173a9c2b6c22947edb7830e5778 |
| extracted Codex binary | 167c0148a849d2444f1b5a7fb5f8bb2de1de5ae13a2a504b833fc765980f5cd9 |

The 0.158.0 source archive is 16,454,306 bytes. The official Linux musl asset
is 108,113,441 bytes and the extracted binary is 286,594,376 bytes. The binary
reports codex-cli 0.158.0.

## Exact compatibility decisions

| Surface | 0.158.0 result | Prodex decision |
| --- | --- | --- |
| Project trust | New portable ProjectTrustLookup tries canonical cwd before original spelling, then repository-root keys. | Keep Super invocation trust scoped and non-persistent. Emit canonical plus original persisted-session workspace keys so prodex s <uuid> and Codex /resume remain zero-prompt. Pin the new project_trust.rs contract in the upstream baseline. |
| Bedrock model catalog | Static provider tests/catalog remove openai.gpt-5.4; GPT-5.5, GPT-5.6 Sol/Terra/Luna, and GPT-6 Sol/Luna remain. | Remove only the obsolete Bedrock compatibility marker. Do not globally remove GPT-5.4 from Prodex's independent OpenAI/Copilot/provider catalogs. |
| Tool-schema compaction | compact_large_tool_schema now accepts max_bytes; DEFAULT_COMPACT_TOOL_SCHEMA_BYTES remains 5,000 and depth/pass policy is retained. | Track the parameterized default in the compatibility baseline; do not reimplement Codex schema compaction in Prodex. |
| Terminal input approval | Enabled by default for elevated commands; runtime-only grants no longer cause unnecessary reviews. | Super remains zero-prompt because Prodex still launches its temporary overlay with approval_policy="never" and sandbox_mode="danger-full-access". Non-Super behavior remains Codex-owned. |
| MCP OAuth client secrets | MCP registration can use pre-registered OAuth client secrets. | Treat as additive upstream-owned MCP configuration; preserve pass-through behavior and do not duplicate secret storage semantics. |
| Exec-server WebSocket auth | Direct exec-server WebSocket connections can require bearer-token authentication. | Treat as additive upstream-owned transport/auth capability. Existing Prodex runtime proxy and app-server boundaries remain unchanged. |
| Image generation/editing | Adds explicit transparent-background control and file-backed conversation images for edits. | Preserve request payloads opaquely across supported provider paths; no Prodex-specific rewrite is required. |
| Command completion events | Completion events include early output and process-launch failures. | Existing additive/opaque event handling remains compatible; no closed event enum is introduced. |
| Existing critical contracts | Responses HTTP/WebSocket, app-server lifecycle, provider auth/cache identity, workspace-routing HTTPS gate, MCP tool-name normalization, quota error classes, remote compaction v2, and thread lifecycle markers all replay successfully. | Keep current Prodex routing, affinity, auth replacement, quota classification, and proxy/session semantics. |

## Project trust verification

Codex 0.158 adds codex-rs/config/src/project_trust.rs. ProjectTrustLookup:

1. receives the original path and an executor-normalized canonical spelling,
2. pushes canonical first when it differs from original,
3. then pushes the original spelling,
4. repeats the same ordered pair for an optional repository root,
5. resolves trust from the final merged projects table.

Prodex's Super trust overlay intentionally supplies both canonical and original
workspace spellings for launch cwd, direct-resume cwd, and recent /resume
workspaces. The trust remains scoped to the temporary Super invocation; Prodex
does not enable a global bypass and does not rewrite the user's persisted Codex
configuration.

## Tool-schema and Bedrock baseline changes

Two previous compatibility markers are intentionally retired:

- openai.gpt-5.4 in codex-rs/model-provider/src/provider.rs, because Codex
  0.158 removes GPT-5.4 only from the bundled Bedrock static manager.
- MAX_COMPACT_TOOL_SCHEMA_BYTES, replaced by
  DEFAULT_COMPACT_TOOL_SCHEMA_BYTES while compact_large_tool_schema accepts a
  caller-provided max_bytes.

No Prodex production implementation depended on either old marker.

## Verification

- Downloaded and SHA-256 verified exact 0.157.1 and 0.158.0 tagged source
  archives; the 0.157.1 hash matches the previously recorded audit.
- Compared extracted tagged trees: 1,224 changed files, 52,257 additions,
  13,525 deletions, and 2 binary entries.
- Replayed all 811 current critical-file and semantic markers against the exact
  0.158.0 source surface, including the new project-trust canonical lookup
  contract: zero missing and zero compatibility diffs.
- Downloaded and SHA-256 verified the official 0.158.0 Linux musl release
  asset; the extracted binary reports codex-cli 0.158.0.
- Ran an isolated official app-server initialize handshake with
  capabilities.experimentalApi=true under a synthetic HOME/CODEX_HOME. It
  returned a valid result with platformFamily=unix and platformOs=linux without
  user credentials or a model turn.
- The compatibility watchdog is in_sync against the updated 0.158.0 baseline,
  and the offline baseline guard plus self-test pass.
- Existing Super trust regressions cover canonical direct-resume cwd and
  pre-trusted /resume session workspaces; these match Codex 0.158's
  canonical-before-original lookup order.

## Prodex 0.433.0

Prodex 0.433.0 targets Codex rust-v0.158.0, advances the audited compatibility
baseline, preserves the zero-prompt Super resume trust contract, and keeps the
new upstream MCP/WebSocket/image/TUI behavior upstream-owned unless Prodex
already owns the corresponding proxy boundary.
