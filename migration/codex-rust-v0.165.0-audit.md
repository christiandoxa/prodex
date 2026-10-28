# Codex rust-v0.165.0 compatibility qualification

Prodex 0.165.0 deliberately succeeds Prodex 0.439.5. This is an authorized
numbering transition to align with Codex, not a rollback of product features.
The exact upstream tag resolves to `360319d8dee62d87e2d59fc0c21805d79519cfed`;
its annotated tag object is `e374836b2728d9b7aa981c2902649e640a2edaa8`.
The prior reference is Codex 0.164.0 at
`6aaac694803004a63c6f393d153c96d22521a6b9`. Exact source and binary digests,
full extracted-tree comparison counts and reviewed file hashes are recorded
in `codex-rust-v0.165.0-source-evidence.json` beside this audit.

## Native contracts and wrapper boundaries

- The built-in `gcp` provider owns Vertex AI endpoint selection, Google Cloud
  credentials, model capabilities and its static model catalog. Prodex now
  recognizes that provider on resume, keeps it outside OpenAI quota/rotation,
  and labels its quota report without claiming Google quota is known. It does
  not import cloud credentials or implement another Vertex transport.
- Named CCA originators have separate catalog files, endpoint headers and TTL
  renewal. Ordinary CLI callers still use `models_cache.json`. Main-agent and
  sub-agent choices must not merge CCA partitions or other accounts' catalogs.
- `model_not_found` is a native terminal error on HTTP 400/403/404 or compatible
  stream errors. Prodex retains upstream status/body and does not manufacture
  quota exhaustion or replay the terminal request. Native retry decisions stay
  in Codex, and existing hard-affinity/no-midstream-replay rules remain.
- Experimental attached forks use `attachToSource` and `attachedToThreadId`.
  These additive fields remain native decisions; existing sessions are not
  attached or migrated automatically by the wrapper.
- Dropped MCP response futures cancel their remote request. Reasoning-effort
  changes are enabled by the native stable feature. Intentional empty native
  namespace descriptions remain empty; only MCP namespaces receive defaults.
- Fresh memory-enabled profiles select v2 only when no explicit version or
  opt-out exists and the old memory directory is absent. I/O errors do not
  authorize migration. Shared-home preparation now includes `memories_v2` through the same guarded
  merge/link owner as v1, without converting formats or overriding native
  memory-version selection. Existing data stays in its own versioned root.
- New directory/session pickers, TUI preferences, reviewer defaults, message
  wake boundaries and marketplace URL validation remain upstream-owned.
  Experimental Guardian features are not enabled by this qualification.

## Verification scope

The offline source guard checks 89 critical files and 114 semantic groups.
Each new mandatory group has a negative marker-removal self-test. The old
namespace-call marker was updated to the actual two-argument source owner,
not waived. Existing required transport/header/authentication checks remain.
Official Linux CLI and app-server assets were checked against release digest
metadata. Isolated version/help, startup-config, JSON-schema and stdio
initialize/thread-list probes succeeded without account access or model turns.
A real-process regression reproduced loss of v2 visibility after switching
profiles before the shared-root fix. Separate v1/v2 content and symlink
containment are covered. Wrapper tests cover native GCP routing and quota isolation, default-catalog
partition isolation, terminal error forwarding, and existing resume boundaries.
All-platform and security qualification remains mandatory in release CI.

## Release numbering and migration

Release history now follows reachable Git ancestry, including annotated tags,
rather than globally sorting version magnitudes. The publish guard permits
only the exact 0.439.5-to-0.165.0 cutover with the immutable old release commit
and the qualified `rust-v0.165.0` baseline. Other version regressions remain
errors. Subsequent aligned releases resume ordinary increasing SemVer.

The update cache uses a separate aligned-generation filename so stale legacy
hints cannot suggest reinstalling 0.439.x. Generic SemVer comparison is not
redefined for other clients, packages or arbitrary version pairs. An already
installed legacy updater cannot be patched by publishing a new binary: users
must run the verified standalone installer explicitly once, using
`sh install.sh --release 0.165.0` or `./install.ps1 -Release 0.165.0`.
Download and verify the installer from the official release before executing
it; do not pipe a remote script directly into a shell. The installer continues
to verify binary checksums and does not install, replace or download Codex.
Existing release tags/assets, user state and running processes are preserved.
