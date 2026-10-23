# Codex rust-v0.164.0 compatibility qualification

Prodex 0.439.0 qualifies the official `rust-v0.164.0` release, published on
2026-10-23 at 04:48:10 UTC. Its annotated tag resolves to commit
`6aaac694803004a63c6f393d153c96d22521a6b9`. The preceding qualified source is
`rust-v0.163.0`, commit `83086188ae0614bd882818ff1aa23f3194962012`.
Exact source and binary identities are recorded in
`codex-rust-v0.164.0-source-evidence.json` beside this audit.

## Reviewed protocol and lifecycle changes

- Prompt cache selection now permits session-derived, custom and omitted keys.
  The Responses `session_id` header remains session-derived independently.
  Prodex must preserve both identities, not synthesize an omitted cache key or
  overwrite a custom key with a session ID. Offline proxy fixtures exercise
  custom and omitted keys on a real local HTTP forwarding path.
- `turn/start` and `turn/steer` accept `responsesapiClientMetadataPatch`.
  Native turn metadata applies an optional replacement before a filtered patch
  and retains reserved continuation metadata. The wrapper does not merge this
  policy itself; unknown fields and native merged metadata remain transparent,
  including across an existing precommit quota-rotation fixture.
- Transient custom-provider model discovery failures retain the last successful
  same-identity catalog. Explicit 401/403 and rejected credentials invalidate
  it. Explicit catalogs still suppress bundled fallback; stale completions
  cannot clear a newer endpoint identity. Old guard descriptions claiming
  invalidation on every failure were corrected, not silently waived.
- Base-instruction world-state ownership moved to `context/world_state/model.rs`.
  Native `ModelInstructionsState` handles initial bases and subsequent model or
  instruction changes. Required source markers follow that owner and preserve
  dedicated developer-item and exact harness-kind identification.
- Native CLI config overrides are collected from root and subcommand scopes
  in order. Prodex launch regressions preserve repeated `-c`/`--config` values
  across root, `exec`, and `resume`, while inline native commands remain prompt
  text rather than wrapper arguments.
- Foreground compaction buffers each attempt until completion before installing
  its summary. Background compaction removes a closed oneshot receiver while
  retaining an empty receiver. Prodex does not manufacture completion, retry
  committed output, or relax continuation affinity.
- Native resume re-resolves compressed rollout siblings during canonicalization
  races and restores persisted thread model settings without crossing a foreign
  fork snapshot or overriding explicit invocation choices. These remain Codex
  lifecycle decisions; local compressed-session regression coverage is retained.

## Native features and ownership boundaries

Presets and inline model/permission commands, Kitty/Sixel image presentation,
enterprise MCP OAuth grants, remote-shell credential masking, Bedrock setup,
Windows proxy prompting, sandbox aliases and symlink execution, and V8 object
handling stay upstream-owned. Prodex does not copy these implementations or
change native permission defaults. Account authentication, tool execution,
remote MCP provisioning and live model inference were not exercised by this
read-only qualification.

The minimum accepted Codex capability floor remains 0.153.2. The qualified
latest-stable reference changes to 0.164.0; this is not an automatic claim that
all older/newer native binaries implement the same feature set.

## Verification and inherited release blockers

The exact-source baseline guard covers 82 critical files and 101 semantic
contracts. Eleven newly mandatory groups have negative self-tests that remove
a decisive marker and verify rejection. Existing capture/replay fixtures remain
mandatory. Prior required markers were updated only after comparing the tagged
sources, including the fallible OAuth-header conversion in security setup.

The official Linux x86-64 musl Codex and app-server assets were SHA-256 checked
against release metadata before execution. Isolated homes with no credentials
passed version/help, `initialize`/`initialized`, `thread/list`, startup-config
inspection with root/subcommand overrides, and experimental JSON-schema export.
The exported start/steer schemas contain the new metadata-patch field. No model
turn or account login was sent, and no installed binary was replaced.

The existing main-branch CI run `39874043773` had the same stale ABI negative
fixture failure on Linux, macOS and Windows: operation 1 had become the supported
chat-request shape validator. The fixture now positively checks operation 1 and
rejects unknown operations -1 and 2, preserving its output canaries. Production
ABI behavior and gates are unchanged. The separate Sonar child-environment
bridge issue is addressed by branch-free accumulation of the returned bypass
bit; compiled Mojo remains the decision owner.

The subsequent source run `39906937663` passed the repaired ABI and Sonar gates
and exposed two additional platform-only fixture assumptions. The optimizer
fixture resolves its trusted temporary root before production's no-symlink
check on macOS; image-scanner expected paths use separate native joins on
Windows. No production filesystem guard or malformed-image assertion is
weakened or skipped.

## Release dependency qualification

The stable tunnel-client reference advances from 0.0.16 to 0.0.17; its minimum
remains 0.0.13. The 25-commit upstream delta was reviewed for CLI/app-server
startup ownership, catalog readiness and transient OAuth discovery retries.
The official Linux binary passed checksum, version and help-only probes without
opening a tunnel or provisioning a resource. Wrapper lifecycle contracts remain
unchanged.

The standalone packaging pin advances from Kiro CLI 2.33.0 to 2.33.1, matching
the official stable manifest. Existing per-platform manifest and digest checks
remain mandatory. This records a packaging freshness check, not an end-to-end
Kiro inference or authentication claim. The six optional-tool references were
also checked against their official release/package sources.
