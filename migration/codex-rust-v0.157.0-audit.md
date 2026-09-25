# Codex rust-v0.157.0 compatibility audit

## Disposition

CODEX_0_157_0_COMPATIBILITY=PRODEX_BASELINE_UPDATE_AND_SUPER_RESUME_TRUST_FIX_REQUIRED.

Codex 0.157.0 is a broad release over 0.156.1. The exact tagged source trees
change 1,086 files with 42,832 insertions and 10,984 deletions. Prodex keeps
upstream-owned daemon, TUI, voice, import, network-policy, and model-catalog
behavior external, while updating compatibility contracts that Prodex observes
at its launch/proxy boundaries.

The release also exposed an older Prodex Super resume regression: a direct
prodex s <session-uuid> launched from a different directory trusted only the
process launch cwd. Codex resumes the persisted thread cwd, so the TUI could
still show its Folder access / Trust and continue prompt. Super now trusts both
the launch workspace and the persisted resumed-session cwd before Codex starts.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.157.0
- Annotated tag object: ac21625ddf7f9dd5f34b2802212cf20295fdff95
- Peeled commit: 00c972ed5d6ff6499317fd41b7f23605b8e6850d
- Release: 0.157.0, non-prerelease, published 2026-09-25T02:31:06Z
- Previous compatibility target: rust-v0.156.1
- Exact tagged-tree comparison: 1,086 changed files, 42,832 additions, 10,984 deletions.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.156.1 source archive | cd1e3d063dcf5e029815b7bb178c9017221ddd12bd49b879b14c1f4b09219a02 |
| rust-v0.157.0 source archive | 10d6b3d485440b6b92d46e20398a940f756bb12a6a98a1daa1932df05937eab7 |
| codex-x86_64-unknown-linux-musl.tar.gz | db3fe3adaa35c50edfb68a988a117782fe3492960fb63d7003eb6748ccc0657b |
| extracted Codex binary | 1a822376d4634ac32dddc030e5117c63359f7f8cd4b1b64382c68190287d0258 |

The official Linux musl asset is 107,842,066 bytes and the extracted binary is
285,340,072 bytes. The binary reports codex-cli 0.157.0.

## Exact compatibility decisions

| Surface | 0.157.0 result | Prodex decision |
| --- | --- | --- |
| Amazon Bedrock GPT-6 | Static Bedrock catalog adds openai.gpt-6-sol and openai.gpt-6-luna; GPT-6 Sol becomes the default while GPT-5.6 entries remain. | Track the exact constants/catalog contract and continue launching Bedrock profiles directly. Do not duplicate the upstream static catalog in Prodex. |
| invalid_prompt | Responses failed events now map invalid_prompt to a distinct InvalidPrompt API error instead of generic InvalidRequest. | Keep invalid_prompt non-quota/non-overload in Prodex; pass it through without profile rotation. Existing error-policy coverage already enforces this class boundary. |
| Internal request metadata | Tool-result metadata and cumulative MCP attribution client metadata are emitted only when the resolved destination is HTTPS OpenAI or an allowed ChatGPT host. | Respect this upstream security boundary. Prodex does not reconstruct metadata that Codex intentionally filters for its loopback custom provider destination. |
| Network policy | HTTP, redirect, SSE, WebSocket, realtime, auth, and app-server paths propagate policy revocation/denial more consistently. | Preserve transport failures transparently and do not reinterpret policy errors as quota. Existing pre-commit rotation boundaries remain unchanged. |
| Thread item lifecycle | ThreadItemEntry adds optional started_at_ms and completed_at_ms. | Preserve additive app-server fields opaquely; compatibility guards pin both fields. |
| Background app server | Eligible interactive sessions can auto-start the background server and offer recovery for incompatible settings. | Upstream-owned. Prodex keeps its session companion/control-plane integration capability-based and does not duplicate daemon lifecycle policy. |
| /import | Available in remote and local background-server sessions. | Upstream-owned passthrough; no Prodex command rewrite. |
| Super direct resume trust | Codex trust UI source is unchanged between 0.156.1 and 0.157.0; trust still depends on the active project path. | Fix Prodex: use existing session-store metadata to read the resumed thread cwd and add it to Super project trust before TUI startup, alongside the launch cwd. No global trust bypass is added. |

## Super direct-resume trust regression

Affected flow:

    cd /some/other/directory
    prodex s <session-uuid>

Before this fix, trusted_workspace_codex_args projected only the process current
directory into Codex project trust. The direct-resume repair already located
the authoritative rollout file before the child process was planned, but its
persisted cwd was not used for trust.

The fix reuses the existing session-store parser, including compressed rollout
support, to read the valid session cwd from that repaired rollout path. Super
then emits one project-trust override containing both unique absolute
workspaces. Relative or unavailable persisted paths are not promoted.

This keeps the Super contract narrow:

- no global hook-trust bypass is introduced;
- hook trust still uses exact hashes plus post-write verification;
- normal non-Super launches do not gain automatic project trust;
- direct resume follows the session's persisted workspace rather than guessing
  from the shell cwd.

## Verification

- Downloaded exact 0.156.1 and 0.157.0 tagged source archives and compared the complete trees.
- Replayed all baseline critical-file and semantic markers against the exact 0.157.0 source tree: 797 markers checked, zero missing.
- Updated the offline baseline guard and its self-test for GPT-6 Bedrock, first-party internal metadata gating, typed invalid_prompt, and thread item lifecycle timestamps.
- Downloaded and SHA-256 verified the official 0.157.0 Linux musl release asset; the extracted binary reports codex-cli 0.157.0.
- Ran an isolated official app-server initialize handshake with experimentalApi=true; it returned a valid 0.157.0 result without real user credentials or a model turn.
- Added a focused Prodex test where launch cwd and persisted resumed-session cwd differ; the generated Super config marks both projects trusted.

## Prodex 0.432.0

Prodex 0.432.0 targets Codex rust-v0.157.0, carries the compatibility baseline
updates above, and fixes zero-prompt project trust for direct Super session
resume across workspaces.
