# Codex rust-v0.159.1 compatibility audit

## Disposition

CODEX_0_159_1_COMPATIBILITY=PRODEX_BASELINE_UPDATED_GPT_6_1_SOL_CONTEXT_ADDED.

Codex 0.159.1 is a focused model-catalog patch over 0.159.0. The exact tagged
source trees change 29 files with 338 insertions and 108 deletions. The release
adds GPT-6.1 Sol as the default bundled model and to Amazon Bedrock Mantle and
Runtime catalogs. Prodex's Responses, WebSocket, provider-auth, session-affinity,
app-server lifecycle, workspace trust, MCP naming, and quota/error contracts remain
compatible without transport or protocol changes.

Prodex already preserves upstream model metadata opaquely, but it derives its own
runtime and Smart Context context budgets. GPT-6.1 Sol therefore joins the existing
large/max-context policy with the upstream 272,000-token base and 872,000-token
maximum context values. The Bedrock catalog itself remains upstream-owned.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.159.1
- Annotated tag object: dd48cd3d094a133f2781ab10343dc665a37e5bc0
- Peeled commit: 8e68a98ef03cdde76d2e6800791ebdf1b3b95b24
- Release: 0.159.1, non-prerelease, published 2026-09-29T20:32:34Z
- Previous compatibility target: rust-v0.159.0
- Exact tagged-tree comparison: 29 changed files, 338 additions, and 108 deletions.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.159.0 source archive | d5fe92c6522bdd3d1f4bb8e74470e6226e80cd5991101c92f587f4a23be8e1ff |
| rust-v0.159.1 source archive | dbc5cad4ea2d7cf52996b05c275313e5f84418b3f3afb6a620aa491eadb6e6a1 |
| codex-x86_64-unknown-linux-musl.tar.gz | 47af8bb41b00eaf3a809c27d5f8357740910373a6a5dedb54f9ee748cedd6851 |
| extracted Codex binary | f1cd7fa4ac866c724122f173d408a95385257cbcc9ac5ce5ddba42669c74ba97 |

The 0.159.1 source archive is 16,567,687 bytes. The official Linux musl asset
is 108,534,949 bytes and the extracted binary is 286,844,264 bytes. The asset
hash matches the GitHub release digest and the binary reports `codex-cli 0.159.1`.

## Exact compatibility decisions

| Surface | 0.159.1 result | Prodex decision |
| --- | --- | --- |
| Bundled default model | Adds visible `gpt-6.1-sol` as the default/highest-priority bundled model. It declares a 272,000 base context window, 872,000 maximum context window, Low default reasoning, Low/Medium/High/XHigh/Max/Ultra reasoning options, text+image input, and Responses Lite. | Preserve the upstream catalog and recognize GPT-6.1 Sol as an 872,000-token large/max-context model when Prodex derives runtime and Smart Context budgets. |
| Amazon Bedrock Mantle catalog | Adds `openai.gpt-6.1-sol`, makes it the default, shifts retained model priorities, and preserves GPT-6 Sol/Luna plus GPT-5.6 compatibility entries. | Track the exact upstream catalog markers and default. Do not duplicate the Bedrock static catalog in Prodex. |
| Amazon Bedrock Runtime catalog | Adds `openai.gpt-6.1-sol` to the runtime allowlist/catalog projection. | Preserve runtime catalog behavior upstream; no Prodex route rewrite is required. |
| Existing critical contracts | All existing Responses HTTP/WebSocket, internal-metadata gate/stripping, provider auth/cache identity, quota/rate/overload classification, project trust, compaction, MCP naming, and app-server lifecycle markers remain present. | Keep existing Prodex routing, affinity, auth replacement, quota rotation, trust, and protocol boundaries unchanged. |

## GPT-6.1 Sol runtime context compatibility

Prodex has two independent context decisions outside the upstream model catalog:

1. Runtime launch decides which OpenAI model families should prefer
   `max_context_window` from `models_cache.json` when present.
2. Smart Context keeps a bounded registry for known model hard caps when no more
   specific runtime observation is available.

Both now recognize `gpt-6.1-sol` and use the upstream 872,000-token maximum.
This mirrors the existing GPT-6 Sol/Luna handling and does not infer a new quota
reserve, pricing rule, or provider-routing policy from the model name.

## Verification

- Downloaded and SHA-256 verified exact 0.159.0 and 0.159.1 tagged source
  archives using the same GitHub tag-archive format used by prior audits.
- Compared exact tagged git trees: 29 changed files, 338 additions, and 108
  deletions.
- Downloaded and SHA-256 verified the official 0.159.1 Linux musl asset; the
  asset digest matches GitHub metadata and the extracted binary reports
  `codex-cli 0.159.1`.
- Ran an isolated official app-server `initialize` handshake with
  `capabilities.experimentalApi=true` under a synthetic HOME/CODEX_HOME. It
  returned a valid result with `platformFamily=unix`, `platformOs=linux`, and
  the expected synthetic Codex home without user credentials or a model turn.
- Replayed all 858 current critical-file and semantic source markers against the
  exact 0.159.1 tagged tree: zero missing files and zero missing markers.
- The offline upstream baseline guard and self-test pass after adding GPT-6.1
  Sol model/Bedrock markers and the 0.159.0 -> 0.159.1 patch record.
- Focused runtime-launch and Smart Context tests cover GPT-6.1 Sol's 872,000-token
  maximum context behavior.

## Prodex 0.434.1

Prodex 0.434.1 targets Codex rust-v0.159.1, adds GPT-6.1 Sol context-budget
compatibility, tracks the new upstream bundled/Bedrock default, and preserves the
0.159.0 transport, security, quota, session, trust, and app-server contracts.
