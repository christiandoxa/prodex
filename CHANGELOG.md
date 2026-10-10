# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.437.3 - 2026-10-10

### Runtime

- Reap the session-owned native app-server process tree (`2efed0b`)
- Prevent admission and quota-state notification deadlock (`370ee38`)
# Prodex 0.437.3

## New Features

No new user-facing features. This patch focuses on runtime recovery and
session-owned process lifecycle correctness.

## Bug Fixes

### Keep quota recovery and admission from deadlocking each other

The admission retry path could acquire the notification mutex and then read
runtime ownership state. Quota, backoff, and binding writers take the opposite
order: runtime state, then the notification mutex. Under local saturation these
two paths could wait on each other indefinitely. New requests and model metadata
refreshes could then time out at the local proxy before account rotation or an
upstream request was reached.

Admission now projects ownership facts before acquiring the notification mutex.
The lost-wakeup recheck performs only the existing atomic/Mojo admission plan,
and ownership is refreshed after each wake. A selection-revision recheck also
prevents notifications arriving between projection and locking from being lost.
The snapshot authorizes admission
only: provider selection still validates the actual account and continuation.
The patch does not increase retry counts, bypass quota, discard hard affinity,
or replay output after it has been committed.

### Stop the complete session-owned app-server process tree

Interactive launches previously used the terminal's process-group policy for the
background app-server companion as well. With an npm launcher in front of the
native executable, terminating that wrapper could leave the native server alive
while its session-local proxy and overlay were being removed.

The companion now always has its own process group, independent of the TUI. Its
owned descendants are terminated together on session teardown; Linux also sets
the existing parent-death signal guard during companion startup. Explicit remote
servers and other user sessions are not taken over or terminated.

## Verification

- Reproduce the admission lock inversion on the previous release before applying
  the fix; verify the second probe does not hold the notification mutex while
  attempting to read runtime state.
- Verify new ownership is observed after a selection-change notification and
  temporary load still backpressures rather than becoming account exhaustion.
- Reproduce a wrapped native app-server surviving its interactive launcher;
  verify the owned native process no longer survives companion teardown.
- Retain real-socket quota-positive-account recovery and precommit/continuation
  regression coverage, including a temporarily busy alternative account.

## Upgrade Notes

Restart Prodex after upgrading to load the patch. A session-local `codex --remote`
socket printed by a terminated launch is not a persistent service endpoint.
Resume the saved conversation through a new Prodex launch instead of reconnecting
to a removed overlay socket. This release does not claim that account rotation can
eliminate model-wide upstream outages or unrelated network failures.

## Changelog

- Remove admission/state notification lock inversion without changing account policy.
- Preserve ownership notifications across the admission retry handoff.
- Reap session-owned app-server descendants before their proxy and overlay go away.
- Retain hard affinity, quota-positive-account recovery, and descriptor safeguards.

Full Changelog: [0.437.2...0.437.3](https://github.com/christiandoxa/prodex/compare/0.437.2...0.437.3)

## 0.437.2 - 2026-10-10

### Runtime

- Preserve portable affinity recovery and release validation (`a5b927b`)
- Keep quota recovery reachable across busy accounts (`57a1ee2`)
- Move compact continuation policy to Mojo (`5f91526`)
- Exhaust websocket attempts correctly across recovery sweeps (`d191200`)
- Map Mojo affinity policy errors at app boundary (`d7cc909`)
- Migrate runtime proxy body limit policy (`5357e12`)

### CLI

- Preserve turn-state owner facts in quota replay eligibility (`77ffdf0`)

### Docs

- Qualify Codex rust-v0.162.1 (`d59ab48`)

### Misc

- Preserve DeepSeek usage output and metadata edge cases (`f71e135`)
- Preserve Gemini data URL and media text contracts (`0b84d7a`)
- Select oldest cookie across bounded ABI chunks without Rust fallback (`fff216f`)

## 0.437.1 - 2026-10-09

### Runtime

- Preserve previous-response recovery precedence (`50f0004`)
- Recover headerless and bound-turn upstream capacity (`bcecbb6`)
- Move summary log formatting to Mojo (`e600780`)

### CLI

- Saturate profile geometry at integer boundaries (`75365cc`)
- Migrate profile screen geometry policy (`9e04540`)

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
