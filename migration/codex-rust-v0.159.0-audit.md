# Codex rust-v0.159.0 compatibility audit

## Disposition

CODEX_0_159_0_COMPATIBILITY=PRODEX_BASELINE_UPDATED_ATTACHMENT_RESUME_FIXED.

Codex 0.159.0 is an additive release over 0.158.0. The exact tagged source
trees change 605 files with 18,710 insertions and 5,820 deletions. Prodex's
Responses, WebSocket, provider-auth, session-affinity, app-server lifecycle,
MCP naming, workspace trust, and quota contracts remain compatible after two
upstream ownership moves: internal-metadata destination gating moves from the
core client into the model-provider boundary, and failed Responses
classification moves into a dedicated SSE error module.

The release also adds opt-in `instant_interrupt` steering and app-server
`thread/items/list` pagination from an exclusive item anchor. Both surfaces
remain upstream-owned and are preserved by Prodex rather than reimplemented.

Separately, the Prodex Super overlay lifecycle is hardened for Codex resume:
pasted-text and media paths are repaired to durable shared attachment storage
before Codex reads a resumed rollout. This covers interrupted runs where the
temporary `.prodex-overlay-*` directory has already been removed.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.159.0
- Annotated tag object: 377f7f557a6bdea0f3a2d26d4d899c66db4789d0
- Peeled commit: 687a119f0fcaace47e1f1abcc77cec6c813fd6da
- Release: 0.159.0, non-prerelease, published 2026-09-29T08:05:42Z
- Previous compatibility target: rust-v0.158.0
- Exact tagged-tree comparison: 605 changed files, 18,710 additions, and
  5,820 deletions.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.158.0 source archive | e0c6f2492a1afad6ace9101b01a85deb58f81c52ee25af351d3b7478425a3558 |
| rust-v0.159.0 source archive | d5fe92c6522bdd3d1f4bb8e74470e6226e80cd5991101c92f587f4a23be8e1ff |
| codex-x86_64-unknown-linux-musl.tar.gz | 6e587a08cb39599816c598b07e17b4bcbf9d41b5c1c0793a4b632b6741e9cbc0 |
| extracted Codex binary | d2752c52353401f7f6efbfcea68796f4f7a3d3e4769f5d1da53fa49d4856b72f |

The 0.159.0 source archive is 16,548,378 bytes. The official Linux musl asset
is 108,526,210 bytes and the extracted binary is 286,750,056 bytes. The binary
reports `codex-cli 0.159.0`.

## Exact compatibility decisions

| Surface | 0.159.0 result | Prodex decision |
| --- | --- | --- |
| Internal tool metadata | Destination gating moves to `ModelProvider::include_internal_metadata`; the core client still clears tool-result metadata when the resolved gate is false. | Move the baseline security check to the provider owner and keep a second client-side stripping check. Do not reconstruct stripped metadata for loopback or custom providers. |
| Responses failure classification | Quota/rate/overload/invalid-prompt classification moves from `sse/responses.rs` to `sse/responses_error.rs`. | Move the semantic baseline owner without changing Prodex quota rotation semantics. Explicit balance and spend-limit codes remain quota errors. |
| Instant interrupt | Adds `Feature::InstantInterrupt` with key `instant_interrupt`, under-development stage, and `default_enabled=false`. | Preserve it as an upstream opt-in. Prodex does not silently force the feature on. |
| Thread item pagination | `thread/items/list` accepts an opaque cursor or `ThreadItemsListAnchor::Item` scoped to a turn. | Preserve the additive JSON-RPC cursor shape opaquely through app-server boundaries. |
| Session and TUI persistence | Blank sessions retain draft state; TUI onboarding and session headers are refreshed. | Keep persistence upstream-owned. Prodex's overlay maintenance now repairs durable attachment paths before resume. |
| Windows process behavior | Codex suppresses unwanted console windows for additional MCP/code-mode/piped-command launches and improves embedded fallback behavior. | Preserve upstream process behavior; no Prodex compatibility shim is added. |
| Existing critical contracts | Responses HTTP/WebSocket headers, remote compaction, provider auth/cache identity, project trust, MCP naming, app-server lifecycle, and attachment RPC methods remain compatible. | Keep current Prodex routing, affinity, auth replacement, trust, and attachment-storage boundaries. |

## Attachment resume verification

Prodex Super uses temporary profile overlays whose `attachments` and
`image_attachments` entries point at durable managed/shared storage. A
terminal interruption can remove the overlay before the normal post-child
maintenance rewrites literal paths already recorded in the rollout.

Prodex now repairs the selected rollout before Codex resume and recognizes JSON
escape boundaries while scanning attachment paths. A path such as
`.../.prodex-overlay-*/attachments/<id>/pasted-text-1.txt\n...` is therefore
rewritten to the durable shared attachment path rather than treating the
escaped newline as part of the filename. The same repair covers media stored in
the attachment tree.

Focused regressions cover:

- a missing old overlay with existing durable pasted-text and image bytes;
- pasted-text and image paths separated by an escaped JSON newline;
- full shared-Codex attachment maintenance across 74 tests;
- existing Super overlay resume behavior.

## Upstream ownership moves

Two previous file-local compatibility checks are intentionally relocated, not
removed:

1. The first-party internal-metadata destination gate moves from
   `codex-rs/core/src/client.rs` into
   `codex-rs/model-provider/src/provider.rs`. The core client still invokes
   the provider gate and clears tool-result metadata when it returns false.
2. Failed Responses classification moves from
   `codex-rs/codex-api/src/sse/responses.rs` into
   `codex-rs/codex-api/src/sse/responses_error.rs`. The explicit
   `insufficient_quota`, balance/spend-limit, rate-limit, slow-down, overload,
   and invalid-prompt classes are preserved.

The offline baseline now guards both new owners and the remaining client-side
stripping callsite.

## Verification

- Downloaded and SHA-256 verified exact 0.158.0 and 0.159.0 tagged source
  archives.
- Compared extracted tagged trees: 605 changed files, 18,710 additions, and
  5,820 deletions.
- Downloaded and SHA-256 verified the official 0.159.0 Linux musl asset; the
  extracted binary reports `codex-cli 0.159.0`.
- Ran an isolated official app-server `initialize` handshake with
  `capabilities.experimentalApi=true` under a synthetic HOME/CODEX_HOME. It
  returned a valid result with `platformFamily=unix` and
  `platformOs=linux` without user credentials or a model turn.
- Replayed the compatibility watchdog after relocating the security/quota
  semantic owners and adding the instant-interrupt/item-anchor markers:
  `in_sync`, zero diffs.
- The offline baseline guard and its self-test are required before release.
- Attachment resume regressions and the full shared-Codex attachment suite pass.

## Prodex 0.434.0

Prodex 0.434.0 targets Codex rust-v0.159.0, preserves upstream opt-in steering
and additive app-server pagination, repairs stale Super attachment references
before resume, and keeps security/quota semantics pinned to their new upstream
owners.
