# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

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
# Prodex 0.437.2

## New Features

Qualifies upstream Codex `rust-v0.162.1` while preserving its native multiline
async-question rendering and explicit CLI feature-override compatibility checks
for shared daemons. The upstream source and transport assumptions are recorded
in the compatibility baseline; Prodex does not duplicate the upstream TUI.

## Bug Fixes

### Recover on quota-positive accounts that are temporarily busy

A quota failure from a bound account could be shown to the user even when another
account had remaining quota. The recovery-eligibility check incorrectly treated
temporary local in-flight load or transport backoff as terminal unavailability.
The existing Mojo retryable-pool policy now distinguishes recoverability from
immediate readiness. Exhausted quota, incompatible auth, unsupported providers,
model-specific quota restrictions, and explicit exclusions are still enforced.

### Wait for the new account after safe full-context replay

A retained soft session preference could keep capacity waiting scoped to the
old, exhausted account after Codex resent complete conversation history. The
wait now preserves hard continuation ownership without trapping the replay on a
soft account preference. Temporary transport cooldowns on another account remain
waitable after a full-context replay. Account B is not used until normal admission
permits it.
Opaque previous-response and turn-state ownership, precommit-only rotation, and
no replay after visible output remain intact.

### Preserve descriptor headroom before state loading

On Unix, Prodex raises a low inherited soft file-descriptor limit toward 8,192,
never above the existing hard limit and never lowering a larger limit. This runs
before state reads and worker startup. It mitigates `Too many open files` from
low inherited limits without discarding state or closing another process's files.
It is not a claim that all descriptor leaks or host-wide resource shortages are
eliminated. Already-running sessions must be restarted to load the patched binary.

### Keep account selection portable across Windows and macOS

The hard-binding conflict ABI now transports its sentinel string view as scalar
address/length arguments instead of a by-value aggregate. This keeps the Rust/Mojo
calling boundary consistent on Win64 without changing ownership decisions or
adding a Rust fallback. A direct ABI regression exercises empty candidate sets,
Unicode sentinels, invalid lengths, and old-version rejection. macOS process
identity checks explicitly retain executable-path verification, and child-launch
validation fixtures now use an absolute path on every supported platform.

### Release validation corrections

The release also rejects empty DeepSeek shell-command arrays and preserves the
correct terminal overload/rate-limit classification after candidate exhaustion.
CI and the full-test workflow require the real Mojo implementation rather than
allowing an implicit feature-off substitute.

## Changelog

- Keep temporary local capacity separate from recoverable quota eligibility.
- Preserve safe full-context account handoff and strict continuation ownership.
- Cover busy-account rotation with real WebSocket and negative-control tests.
- Add isolated descriptor-exhaustion, soft-limit preservation, and child-process inheritance regressions.
- Qualify Codex `rust-v0.162.1` and synchronize standalone release metadata.

Full Changelog: [0.437.1...0.437.2](https://github.com/christiandoxa/prodex/compare/0.437.1...0.437.2)

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

## 0.435.3 - 2026-10-04

### Runtime

- Backpressure local saturation (`8aaf4e9`)
