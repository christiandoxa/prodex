# Codex rust-v0.160.0 compatibility audit

## Disposition

CODEX_0_160_0_COMPATIBILITY=PRODEX_BASELINE_UPDATED_NO_MODEL_TRANSPORT_CHANGE_REQUIRED.

Codex 0.160.0 is a broad feature release that diverges from the current Prodex
0.159.3 compatibility target. The exact tagged source archives differ in 362
files with 14,664 additions and 2,380 deletions. GitHub's tag comparison reports
0.160.0 as 56 commits ahead and 5 commits behind 0.159.3, so this audit compares
the exact 0.159.3 and 0.160.0 trees rather than relying only on the upstream
0.159.0-to-0.160.0 release changelog.

The existing Prodex transport, auth, quota, session, app-server, and 0.159.3
security-reminder boundaries remain present in 0.160.0. No Prodex model-routing
change is required. The compatibility baseline is extended to guard four new
0.160.0 behavior families that intersect Prodex's runtime assumptions:
authoritative explicit provider catalogs, provider/history restoration,
projectless workspace defaults plus saved permission restoration, and pending
subagent environment inheritance.

## Provenance

- Repository: https://github.com/openai/codex
- Tag: rust-v0.160.0
- Annotated tag object: 79b1b666f2e8551f8abbbca34957227f67f3f553
- Peeled commit: a956835d020762cb2b570053af06f643a11c0ecc
- Release: 0.160.0, non-prerelease, published 2026-10-01T20:19:13Z
- Previous compatibility target: rust-v0.159.3
- Exact tagged-tree comparison: 362 changed files, 14,664 additions, and 2,380 deletions.
- Ancestry comparison: 56 commits ahead and 5 commits behind rust-v0.159.3.

Exact source and binary hashes:

| Artifact | SHA-256 |
| --- | --- |
| rust-v0.159.3 source archive | 7a13f939c8b5ea4e5fb1434f0b4a5ea3f4c85b22a4782818cb28a43ed0deac58 |
| rust-v0.160.0 source archive | e9ba182239bb984f1d24a57c8b3abb3da127e0a108e5f17d7a9038688cf4369c |
| codex-x86_64-unknown-linux-musl.tar.gz | 306865417d4ee7a927785852910a527f41e1e159add390ac5ae3accb67d44a13 |
| extracted Codex binary | 12eb3e81114588aca3b7998f4f19e8997b056aca08e57a7ca7c8a3ec8c652aad |
| codex-app-server-x86_64-unknown-linux-musl.tar.gz | d0a05909836d4a73634641b89c7a62fd99c0978cdfe7ae9b8b631e9ee6d9da46 |
| extracted app-server binary | d9244ef26a5193aec701e61c2f9280d44040efbedb9c16b4c0c6f41e354bcdd3 |

The 0.159.3 source archive is 16,577,987 bytes and the 0.160.0 source archive
is 16,677,782 bytes. The official Linux musl CLI asset is 109,304,578 bytes
and extracts to 289,101,384 bytes. The official app-server asset is 84,642,004
bytes and extracts to 223,271,000 bytes. Both asset hashes match GitHub release
metadata. The CLI reports codex-cli 0.160.0.

## Exact compatibility decisions

| Surface | 0.160.0 result | Prodex decision |
| --- | --- | --- |
| Explicit provider catalogs | OpenAiModelsManager::with_provider_catalog() switches to CatalogSource::ExplicitProvider, clears bundled seed models, suppresses bundled fallback, and clears in-memory/cache models after refresh failures so stale entries are not revived. | Keep Prodex provider routing/catalog ownership unchanged, but guard the upstream authoritative-catalog contract so unsupported bundled models cannot leak into explicit provider catalogs. |
| Provider defaults and history | TUI provider selection honors managed requirements over explicit invocation choices; omitted provider choices allow workspace-effective provider resolution for thread creation, resume, fork, and history. | Preserve explicit Prodex provider overrides when supplied. Treat omission as upstream-owned default resolution and guard history/provider restoration semantics. |
| Projectless sessions | Eligible local projectless folders may receive workspace-write defaults only when no project layer/trust/managed override conflicts, roots match the cwd, execution is local, and policy permits the workspace profile. | Do not force a project/trust decision from Prodex. Keep the overlay boundary capability-based and guard the upstream projectless eligibility conditions. |
| Resume/fork permissions | Direct launch choices are tracked separately from omitted choices; omitted permission/profile/root fields allow app-server to restore the destination task's saved settings. | Keep Prodex's explicit overrides explicit, and leave omitted fields to upstream restoration. Guard this distinction. |
| Pending subagent environments | Spawn/resume passes inherited environment snapshots into child sessions. Children follow the owner's first ready/failure result only while still pending, and do not overwrite a child-side configuration that already won. | Keep Prodex sub-agent process/environment boundaries unchanged and guard the upstream first-result inheritance semantics. |
| Existing security reminder boundary | The 0.159.3 authenticated /wham/security-setup reminder identity/origin checks remain present. | Continue keeping chatgpt_base_url Codex-owned for account/read bootstrap traffic while model traffic remains on the Prodex provider path. |
| Other 0.160.0 changes | Reconnect queue recovery, Guardian context, SQLite/log maintenance, plugin caching, Windows sandbox fixes, and command-center pagination do not alter Prodex model proxy contracts. | No Prodex routing change. These remain upstream-owned implementation behavior. |

## Verification

- Downloaded and SHA-256 verified the exact 0.159.3 and 0.160.0 tagged source
  archives and compared their extracted trees: 362 changed files, 14,664
  additions, and 2,380 deletions.
- Replayed the complete compatibility baseline against rust-v0.160.0:
  61 critical files with 564 required source markers plus 64 semantic checks
  with 414 semantic source markers. Zero Codex compatibility diffs remain.
- The live upstream watchdog reports only unrelated Claude Code release metadata
  drift; Codex release metadata and compatibility markers match the new baseline.
- Verified the official 0.160.0 Linux musl CLI and app-server assets against the
  SHA-256 digests published in GitHub release metadata.
- Verified the extracted CLI reports codex-cli 0.160.0.
- Ran an isolated official 0.160.0 app-server initialize handshake with
  capabilities.experimentalApi=true under synthetic HOME/CODEX_HOME. It
  returned platformFamily=unix, platformOs=linux, and the expected synthetic
  Codex home without credentials or a model turn.
- Added explicit critical-file and semantic guards for authoritative provider
  catalogs, provider/history restoration, projectless defaults, saved permission
  restoration, and pending subagent environment inheritance.
- Retained all existing 0.159.3 account-security identity/origin guards.

## Prodex 0.435.0

Prodex 0.435.0 targets Codex rust-v0.160.0. It advances the audited upstream
baseline without changing the existing Prodex model transport or profile
rotation behavior.
