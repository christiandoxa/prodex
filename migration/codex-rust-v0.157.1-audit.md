# Codex rust-v0.157.1 compatibility audit

## Disposition

CODEX_0_157_1_COMPATIBILITY=PRODEX_BASELINE_PATCH_UPDATE_AND_DIRECT_RESUME_PROVIDER_FIX_REQUIRED.

Codex 0.157.1 is a focused patch over 0.157.0. The exact tagged source trees
change 10 files with 441 insertions and 55 deletions. The runtime changes are
Windows process-lifecycle and console-suppression fixes plus MCP test timing;
none of Prodex's pinned Linux transport, session, provider, model, or app-server
contract files change.

The same Prodex release also fixes a direct Super resume regression that is
independent of the upstream patch. Governed OpenAI sessions persist Codex's
internal model_provider value prodex-openai-governed-http. Plain Codex /resume
already accepts that persisted session, but prodex s <session-uuid>
pre-resolved the provider through Prodex's registry and rejected that internal
identifier before Codex started. The registry now resolves the internal bridge
identifier to canonical OpenAI for persisted model-provider identity only; it
does not become a user-facing provider alias.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.157.1
- Annotated tag object: ac0e23e5232692b95268583c8278c50b8c436d2b
- Peeled commit: 36650394c5b38c2990ccf2a3457165ca3e9d9726
- Release: 0.157.1, non-prerelease, published 2026-09-26T01:02:31Z
- Previous compatibility target: rust-v0.157.0
- Exact tagged-tree comparison: 10 changed files, 441 additions, 55 deletions.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.157.0 source archive | 10d6b3d485440b6b92d46e20398a940f756bb12a6a98a1daa1932df05937eab7 |
| rust-v0.157.1 source archive | e3df5f38129e566723a9b762ef021019aedc830b06a57624d78a03a59b6a9d55 |
| codex-x86_64-unknown-linux-musl.tar.gz | e98c1e8e028e8137fa2d2415c82ec58e7b3701a627e3554aace5b3ca31454af2 |
| extracted Codex binary | 3e2584f3f3829a43a0495011a1cecb2facbe64a2403e2b682351fd9c2983f970 |

The official Linux musl asset is 107,868,387 bytes and the extracted binary is
285,340,072 bytes. The binary reports codex-cli 0.157.1.

## Exact compatibility decisions

| Surface | 0.157.1 result | Prodex decision |
| --- | --- | --- |
| Windows app-server daemon breakaway | Daemon startup no longer rejects residual outer Job Object membership after a successful inner-job breakaway probe. | Preserve upstream behavior; Prodex does not duplicate Windows daemon job policy. |
| Windows daemon stdio | Managed Windows launches clear inheritance on launcher standard handles so detached daemons do not keep caller pipes open. | Preserve upstream process semantics and do not wrap the daemon with competing handle policy. |
| Windows code-mode host and local MCP | Child launches use CREATE_NO_WINDOW; MCP startup-grace tests allow more time on slow runners. | No Prodex protocol change. Keep MCP and code-mode launch surfaces upstream-owned. |
| Linux transport/session/provider contracts | No tracked source change from 0.157.0. | Keep existing Responses, WebSocket, app-server, provider-auth, model-cache, affinity, and rotation contracts unchanged. |
| Super direct resume provider identity | Persisted governed OpenAI sessions use prodex-openai-governed-http, which Prodex previously treated as unsupported during prodex s <uuid> preflight. | Resolve that persisted internal model-provider ID to canonical OpenAI while keeping it out of user-facing aliases. Unknown provider IDs still fail closed. |

## Direct Super resume regression

Affected flow:

    prodex s <session-uuid>

for a session whose report contains:

    model_provider = "prodex-openai-governed-http"

The runtime proxy intentionally installs that Codex provider ID when OpenAI
model traffic is forced through Prodex governance. Codex persists it in the
session metadata. A later direct Super resume consults the persisted report
before launch so provider binding can be enforced. The provider registry knew
the external bridge IDs (prodex-anthropic, prodex-gemini, and others) but did
not know the governed OpenAI bridge ID, so preflight failed with unsupported
provider identity.

The fix adds only a model-provider identity mapping:

    prodex-openai-governed-http -> OpenAI

The ordinary provider alias parser still rejects that internal name. This keeps
CLI provider selection canonical while allowing a persisted Prodex-generated
session to resume through the same governed OpenAI path.

## Verification

- Downloaded and SHA-256 verified exact 0.157.0 and 0.157.1 tagged source
  archives.
- Compared the exact tagged trees: 10 changed files, 441 additions, 55
  deletions, with no binary diff entries.
- Replayed all 797 current critical-file and semantic markers against the exact
  0.157.1 source tree: zero missing.
- Downloaded and SHA-256 verified the official 0.157.1 Linux musl release
  asset; the extracted binary reports codex-cli 0.157.1.
- Ran an isolated official app-server initialize handshake with
  capabilities.experimentalApi=true under a synthetic HOME/CODEX_HOME; it
  returned a valid 0.157.1 result without user credentials or a model turn.
- The provider-core registry test confirms that
  prodex-openai-governed-http resolves to OpenAI only as a model-provider ID.
- The runtime resume test confirms the same persisted identity no longer hits
  the unsupported-provider error while unknown identities still fail closed.
- A source-built prodex s --dry-run against the originally failing local
  governed-OpenAI session completed launch planning successfully, retained the
  session UUID, and generated the governed OpenAI runtime proxy provider.

## Prodex 0.432.2

Prodex 0.432.2 targets Codex rust-v0.157.1, preserves the upstream patch
behavior, and restores direct UUID resume for governed OpenAI Super sessions.
