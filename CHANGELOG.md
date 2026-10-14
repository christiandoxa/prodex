# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.437.13 - 2026-10-15

### Runtime

- Preserve terminal output and recover observer pressure (`bf2cc2b`)

### Docs

- Record login authority coverage (`980c21d`)
- Record extended Mojo wave (`e6c410a`)
- Record Mojo 75 campaign wave (`8d4e04e`)
- Record websocket frame checkpoint (`693bb73`)
- Record status parser checkpoint (`bbc9646`)

### Deps

- Update rustls for October security advisories (`a2bd78c`)

### Misc

- Audit RTK 1.0.1 release reference (`35f5b76`)
- Migrate broker continuity metrics reducer (`229ec56`)
- Align stream JSON validation with serde (`54235b5`)
- Restore ownership manifest (`1087b5d`)
# Prodex 0.437.13

## New Features

No new CLI flags or saved-session storage changes are required.

## Bug Fixes

### Preserve session output after terminal teardown

The post-child terminal cleanup selects the normal screen without restoring
an obsolete saved cursor. Repeating the previous DEC 1049 reset after Codex
had already exited its alternate screen restored the old cursor on VTE,
causing even the plain-text footer to overwrite native output. A real VTE
reproduction demonstrated the overlap and verified the idempotent DEC 1047
handoff. Keyboard, mouse and paste cleanup and queued terminal input remain
preserved. Session IDs and resume commands remain complete, Mojo-formatted
plain text. No main-screen clear or scrollback purge is introduced.

### Keep the frontend alive during observer descriptor pressure

Temporary descriptor exhaustion while reading the launch-owned marker no
longer propagates as a fatal child-wait error. The monitor releases only its
own optional SQLite connection and retries after a bounded delay, keeping
its identity and unread rollout cursor. SQLite availability is checked
before advancing that cursor, so an unavailable goal store cannot consume
a terminal failure without recovery. Unknown observation state authorizes
neither replay nor account rotation. Invalid identities and non-transient
errors still fail explicitly.

An isolated low-RLIMIT test fills the child's descriptor table, checks
that no termination is requested, releases pressure, recovers the unread
failure exactly once and verifies stable descriptor usage across repeated
polls. This addresses the demonstrated fatal observer behavior; the original
exited process's descriptor consumers were unavailable for leak attribution.
It does not claim to eliminate every possible source of descriptor pressure.

### Recover a terminal WebSocket idle timeout

The exact native message `stream disconnected before completion: idle timeout
waiting for websocket`, recorded as `codex_error_info=other`, is now recognized
by the compiled Mojo transport-error classifier. Existing terminal-event,
root-session, acceptance, paused/completed-goal, budget and cancellation
checks remain in force. Intermediate retry notifications do not interrupt
the frontend; acknowledged or ambiguous mutations are not duplicated. This
fixes recovery eligibility, not an upstream service's underlying timeout.

### Security and release validation

The locked TLS dependency is updated to rustls 0.23.46, resolving the four
October 14 RustSec advisories found by the existing supply-chain gate.
The prior main-branch size and quality failures are addressed by mechanical
adapter splits and a smaller quota-input projection function. Ownership
fixtures now distinguish source-only cleanup from the frozen release's
migration volume without changing the denominator or minimum requirement.
The multi-sweep Windows fixture retains a finite request-local deadline;
separate watchdog tests still prove expiry and cancellation behavior.

## Changelog

Focused real-Mojo checks passed: 18 child-process/terminal lifecycle tests,
49 usage-limit recovery tests, 18 session-recovery tests and the direct
WebSocket classifier regression. Coverage includes low file-descriptor
limits, retry backoff, unread-cursor preservation, blocked versus paused
goals, complete session IDs and exact-message negative cases.

The full Mojo-core library suite passed 397 tests; the quota library passed
95 tests. Node tests reported 278 passes and one existing skip. Workspace
Clippy passed with all targets/features and warnings denied. The normal
source-size limits remain unchanged (32 near-limit files); the obsolete
quota adapter allowlist entry is removed rather than widened. Cargo audit
reported zero vulnerabilities after the TLS update.

The release freshness checkpoint also re-audits RTK 1.0.1 against the exact
upstream commit/tree, checksum manifest and five platform archives. The OSS
Linux edition passes version, bypass and legacy proxy alias smokes with
telemetry and tracking disabled. The minimum compatible version is unchanged
and no installed external tool is upgraded by this release.

Full Changelog: [0.437.12...0.437.13](https://github.com/christiandoxa/prodex/compare/0.437.12...0.437.13)

## 0.437.12 - 2026-10-14

### Misc

- Keep saved session IDs readable after Super exits (`f35210d`)

## 0.437.11 - 2026-10-14

### Runtime

- Persist session release fences across resume and rotation (`da0fabc`)
- Accept trimmed header tuple bounds (`b4dea3f`)
- Migrate marker aggregation to Mojo (`d37d0f9`)

### CLI

- Move panel error line normalization to Mojo (`2affd22`)

### Docs

- Record latest migration checkpoints (`5980212`)

### Misc

- Preserve complete marker facets and simplify ABI projection (`610cf97`)

## 0.437.10 - 2026-10-13

### Misc

- Preserve pasted goal media across overlay exit and resume (`0548b3f`)

## 0.437.9 - 2026-10-13

### Runtime

- Integrate native capacity recovery safeguards (`bd3c1d5`)
- Preserve root session recovery ownership and cursor (`bca7e83`)
- Keep Responses heartbeats uncommitted (`6e49493`)

## 0.437.8 - 2026-10-13

### Runtime

- Bound recovery waits without reopening live sessions (`ef7cf36`)
- Recover native request timeouts after compaction (`cc146bb`)
- Plan runtime route circuit actions (`dec6d74`)
- Migrate websocket recovery predicates (`3421a40`)

### Deps

- Align Kiro stable native release probe (`082bfc9`)
- Verify Ponytail 5.2.0 compaction compatibility (`38112e8`)
- Audit Ponytail 5.1.1 stable source (`ef94261`)
- Update quinn-proto past peer-triggered panic advisories (`f7e2028`)

### Misc

- Migrate Gemini thought signatures (`eacb848`)

## 0.437.7 - 2026-10-12

### Runtime

- Recover terminal native threads without relaunch (`c554fc0`)
- Preserve Codex context defaults and recover native threads in place (`82d311f`)

## 0.437.6 - 2026-10-12

### Runtime

- Keep large-session recovery armed after compaction (`7151012`)

### Deps

- Bump rust from 1.98.1-bookworm to 1.99.0-bookworm (`db5cb81`)
- Bump the cargo group with 6 updates (`ea659d2`)

### Misc

- Use Rust 1.99 atomic update API (`c757a6a`)
- Merge Dependabot PR #111 for 0.437.6 (`4ac9273`)
- Merge Dependabot PR #110 for 0.437.6 (`02eb770`)
- Merge Dependabot PR #109 for 0.437.6 (`0164486`)

## 0.437.5 - 2026-10-11

### Runtime

- Stop wrapped native TUI before retiring its endpoint (`1fe2262`)

### CLI

- Preserve long activation profile names (`386db91`)

### Misc

- Repair native goal attachment aliases before resume (`494c414`)

## 0.437.4 - 2026-10-11

### Runtime

- Reset recovery evidence on native task start (`886bcc5`)
- Recover native Codex task completion failures (`96cf9ab`)

## 0.437.3 - 2026-10-10

### Runtime

- Reap the session-owned native app-server process tree (`2efed0b`)
- Prevent admission and quota-state notification deadlock (`370ee38`)

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
