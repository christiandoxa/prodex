# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.437.0 - 2026-10-09

### Runtime

- Retain streamed rate-limit retry advice in Mojo (`c0f3b1b`)

### CLI

- Restore profile management capacity guard (`2f58db6`)
- Migrate concurrency choices to Mojo (`822eaf7`)

### Docs

- Record compact and sub-agent ownership (`1a2197a`)
# Prodex 0.437.0

## New Features

### Codex 0.162.0 compatibility

Qualifies the official `rust-v0.162.0` source at
`c1382380de69521303b416720a52f42d51af6248`. The compatibility audit replays 1,238
source markers across 77 critical files and 86 semantic checks against the exact
tagged source, including moved provider-capability ownership, streamed retry
advice, response phases, remote MCP environment handling, and additive app-server
lineage contracts. Historical qualification evidence remains intact.

The latest-tested reference is not a mandatory installed Codex update. The
capability-based minimum remains 0.153.2; this release does not replace an
installed Codex executable or bypass upstream account or feature eligibility.

## Bug Fixes

### Streamed rate-limit retry advice

Fixes a production-boundary defect where an SSE or WebSocket rate-limit failure
could use a one-second delay from the message even when the structured
`Retry-After` header requested five seconds. A regression reproduced that exact
failure before the patch.

The canonical Mojo planner now selects structured retry advice from
`response.failed.response.error.headers` and WebSocket error headers, preserving
nested/outer precedence, upstream JSON header insertion behavior, zero delays,
HTTP dates, invalid-header handling, and the existing 300-second local delay cap.
Rust supplies decoded JSON, buffers, and the host clock/date codec, not a duplicate
retry-policy fallback. The real SSE and WebSocket inspection paths are covered.

Post-commit no-replay behavior, hard affinity, account quota classification, and
admission limits remain unchanged. Generic overload/profile-rotation scheduling
is not redesigned by this rate-limit fix.

## Provider configuration boundaries

Four credential-free executable tests pass real Prodex launch arguments to the
official Codex 0.162.0 app-server and verify the parsed provider configuration.
They cover supported local `--url` and governed OpenAI providers, inline and leaf
capability overrides, `external_web_access`, and `remote_compaction` values `v2`
and `unsupported`. Explicit opt-outs remain intact.

Reserved built-in provider overrides remain rejected by Codex. Arbitrary custom
provider IDs are not silently enabled in Prodex Super; use the supported `--url`
or `--provider` entry points, or plain Codex. Upstream TUI/worktree/Daybreak
features remain subject to Codex's own support and eligibility rules.

## Tooling and release qualification

Ponytail 5.1.0 is the newly qualified optional-tool reference, verified against
its official commit, archive, and complete tree digest. Existing minimum-version
and legacy-manifest compatibility is preserved. No upstream hooks or installers
were executed during this audit.

Oversized modules and test groups inherited from earlier migration checkpoints
were split by domain. Duplicate imports were removed. Size, ownership,
no-fallback, lint, and test assertions were not weakened to make the release pass.

Local qualification includes 408 passing runtime-proxy tests on the final
runtime fix, full-workspace/all-target/all-feature Clippy with warnings denied,
static guards, and 253 passing Node tests with one existing skipped case.
Official Codex CLI and app-server archive hashes match the GitHub asset digests;
isolated initialization and configuration RPCs pass without live model turns.

## Distribution

Standalone binaries remain available for Linux, macOS, and Windows on x86_64 and
ARM64. Versionless installation URLs, manifests, checksums, and the standalone
GitHub release workflow remain the distribution surface. The official Mojo
production-share target and non-regression rules are unchanged.

## Changelog

- Qualify Codex 0.162.0 at its immutable release commit.
- Preserve supported provider capabilities through actual launch boundaries.
- Honor structured streamed rate-limit retry advice through the canonical Mojo planner.
- Qualify Ponytail 5.1.0 and restore strict inherited size/lint hygiene.
- Synchronize standalone release metadata for Prodex 0.437.0.

Full Changelog: [0.436.1...0.437.0](https://github.com/christiandoxa/prodex/compare/0.436.1...0.437.0)

## 0.436.1 - 2026-10-08

### Runtime

- Expose shared JSON ABI to runtime-only builds (`c9ee979`)
- Recover delayed SSE capacity failures without retaining permits (`bba3a25`)

### Misc

- Point scalar reachability to extracted ABI (`1d6c8d1`)
- Preserve long shell arguments in Mojo parser (`5b81c6b`)
- Accept large app-server pages through Mojo ABI (`51f66d7`)

## 0.436.0 - 2026-10-08

### Misc

- Preserve Codex 0.161 program arguments in Mojo (`eac2f58`)

## 0.435.9 - 2026-10-08

### Runtime

- Keep queued fanout out of upstream retry budgets (`cfa0681`)

## 0.435.8 - 2026-10-07

### Runtime

- Wait through cold-start probe slices (`378ca83`)

### Misc

- Add OpenAI model effort picker (`6842e68`)

## 0.435.7 - 2026-10-07

### Runtime

- Recover post-compaction quota affinity (`5f5eb4d`)

### Deps

- Qualify Kiro CLI 2.28.0 (`0a55345`)
- Qualify tunnel-client 0.0.16 (`131c41c`)

### Misc

- Preserve large response metadata (`5ca2916`)

## 0.435.6 - 2026-10-06

### Runtime

- Satisfy compact recovery clippy (`466175b`)
- Close 0.435.6 CI regressions (`c0644d6`)
- Keep usable profiles transparent (`ba72da5`)

### Docs

- Record runtime-doctor Mojo proof (`61fc70d`)

### Deps

- Bump tokio-rustls from 0.26.5 to 0.26.6 in the cargo group (`d122561`)

### Misc

- Merge pull request #101 from christiandoxa/campaign/mojo-capsule-order-split-20261004 (`07a1995`)
- Merge pull request #104 from christiandoxa/campaign/mojo-wave-observability-20261004 (`bc0690f`)
- Merge pull request #108 from christiandoxa/dependabot/github_actions/github-actions-6de4297576 (`3c12233`)
- Merge pull request #107 from christiandoxa/dependabot/cargo/cargo-bbe38bb5bb (`c3f5edb`)

## 0.435.5 - 2026-10-05

### Runtime

- Replay hard-affinity quota blocks (`6515c54`)

### Docs

- Record integrated Mojo migration validation (`6150fa1`)

### Misc

- Merge pull request #105 from christiandoxa/campaign/mojo-local-rewrite-policy-20261004 (`d1d7a32`)
- Merge pull request #106 from christiandoxa/campaign/mojo-log-load-aggregate-20261004 (`2556f62`)
- Merge pull request #102 from christiandoxa/campaign/mojo-wave-config-20261004 (`f546cec`)
- Merge pull request #103 from christiandoxa/campaign/mojo-wave-policy-20261004 (`69d079b`)

## 0.435.4 - 2026-10-04

### Runtime

- Honor request-local quota exclusions (`4f3e21a`)
- Reselect profiles after recovery wait (`12d69a1`)
- Keep retryable profiles alive past precommit budget (`0934e00`)

## 0.435.3 - 2026-10-04

### Runtime

- Backpressure local saturation (`8aaf4e9`)

## 0.435.2 - 2026-10-03

### Runtime

- Stop exhausted same-request probe races (`063f81f`)
- Keep auto-rotate alive while quota remains (`02997e4`)

### CLI

- Delegate profile health circuit timing (`a00b4dc`)

### Docs

- Record compact-exit checkpoint (`6b67cd6`)
- Record sub-agent renderer wave (`c5af517`)
- Record provider surface migration checkpoint (`4e175fc`)

## 0.435.1 - 2026-10-02

### Docs

- Add 0.435.1 release notes (`9c6bc5d`)

### Misc

- Align Mojo fallback self tests (`99c68d8`)
- Align provider defaults with latest models (`ecacd64`)
- Retain active Copilot GPT-5.6 models (`b2e0611`)
- Sync latest provider models (`e6965ee`)
