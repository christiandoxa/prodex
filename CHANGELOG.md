# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.437.1 - 2026-10-09

### Runtime

- Preserve previous-response recovery precedence (`50f0004`)
- Recover headerless and bound-turn upstream capacity (`bcecbb6`)
- Move summary log formatting to Mojo (`e600780`)

### CLI

- Saturate profile geometry at integer boundaries (`75365cc`)
- Migrate profile screen geometry policy (`9e04540`)
# Prodex 0.437.1

## New Features

No new CLI surface. This patch release hardens existing Responses precommit
recovery while retaining Codex 0.162.0 compatibility.

## Bug Fixes

### Preserve precommit inspection after Codex falls back to HTTP

A Responses request with `stream: true` could receive an HTTP 200 SSE body without
`Content-Type`. Prodex previously treated every such response as unary success,
allowing an early `server_is_overloaded` event to reach Codex without running the
SSE precommit recovery path. Normal SSE fixtures did not expose the gap because
they always declared `text/event-stream`.

The canonical Mojo framing decision now considers the original request's stream
flag when the response MIME header is absent or blank. Explicit response MIME
types keep their previous behavior. Upstream headers and response bytes remain
unchanged; the fix does not fabricate a MIME header, a success, or a quota error.

### Retry transient capacity safely within a bound turn

A later HTTP sample in the same turn may carry a profile-bound turn-state even
without a previous-response ID. These requests previously passed an overload
straight through because rotating their continuation to another account is
unsafe. They now retry the same owner before commit, using a Mojo-owned policy
with at most five retries and a 60-second retry-planning window. Existing network
timeouts still bound each upstream attempt independently.

The delay uses exponential backoff and bounded jitter, and preserves upstream
`Retry-After`. Advice beyond the remaining planning window is not shortened to
force an early retry. Failed attempts release their in-flight admission slot
before waiting and reacquire it before retrying. Exhaustion preserves the
upstream failure; visible output is never replayed. Fresh requests retain the
existing eligible-profile rotation path.
Previous-response-ID repair retains its established full-history retry signaling;
the additional same-owner retry path is for turn-state-only HTTP samples.

### Release qualification and hygiene

Credential-free production-path regressions reproduce the missing-MIME failure
and the same-turn recovery gap before their fixes. Coverage includes successful
and failed headerless streams, explicit JSON and unary requests, precommit
rotation, same-owner continuation recovery, retry exhaustion, admission release,
long retry advice, and no replay after visible output. Direct Mojo checks cover
framing precedence and retry-policy boundaries.

Oversized audit-policy tests, response-forwarding classifier tests, and the
recovery-batch ABI adapter are split into focused modules without changing their
public behavior or weakening size and no-fallback guards. Existing Codex 0.162.0
qualification and the capability-based minimum remain unchanged.

## Changelog

- Inspect requested SSE streams even when upstream omits the MIME header.
- Retry precommit overloads on the bound owner without cross-account replay.
- Respect upstream retry advice, local retry bounds, and admission-slot lifetimes.
- Register missing-header, same-turn, and post-commit regressions in CI.
- Restore strict module-size and lint qualification without weakening guards.
- Synchronize standalone release metadata for Prodex 0.437.1.

Full Changelog: [0.437.0...0.437.1](https://github.com/christiandoxa/prodex/compare/0.437.0...0.437.1)

## 0.437.0 - 2026-10-09

### Runtime

- Retain streamed rate-limit retry advice in Mojo (`c0f3b1b`)

### CLI

- Restore profile management capacity guard (`2f58db6`)
- Migrate concurrency choices to Mojo (`822eaf7`)

### Docs

- Record compact and sub-agent ownership (`1a2197a`)

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
