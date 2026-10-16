# Changelog

Generated from conventional commits. Run `npm run changelog` to refresh.

## 0.438.0 - 2026-10-16

### Runtime

- Accept signed Windows exit status in Mojo recovery (`a31ef6f`)
- Keep local proxy listeners alive under descriptor pressure (`d28ba19`)
- Migrate runtime affinity admission (`74a7573`)
- Migrate websocket precommit hold state (`6dd340e`)
- Migrate runtime launch exit cancellation (`995625d`)
- Migrate provider runtime metadata (`a4a02ce`)
- Migrate runtime profile jitter (`dd15901`)
- Migrate runtime last-chance candidate policy (`45b8548`)
- Migrate runtime log follow planning (`6b0529b`)
- Migrate runtime quota summary ordering (`afa19e6`)
- Migrate websocket terminal frame policy to Mojo (`271133a`)
- Migrate broker continuation metrics (`dc5cea9`)

### CLI

- Move profile import identity email policy (`5576854`)
- Move Super normalization policies to Mojo (`a9281db`)
- Migrate quota watch table columns (`a7f620f`)

### Claude

- Migrate MCP stdio first-line classification (`ccd6f84`)

### Misc

- Clarify source digests and isolate Copilot validation (`c2e82b5`)
- Qualify Codex 0.163 native retry and compaction (`6073f85`)
- Migrate usage-limit recovery selection (`7087fdc`)
- Migrate super-expose rollout filename policy (`72eb410`)
- Migrate throughput display policy (`ffaaa6c`)
- Migrate Kiro tool control shaping (`fcbc88d`)
- Migrate stale login retention (`b4c19f1`)
- Batch Gemini response part planning (`0abe987`)
- Migrate Copilot import candidate resolution (`a3b7b13`)
- Migrate status resource aggregation (`ab2bd18`)
- Migrate panel field row layout (`97f448c`)
- Migrate Gemini tool validation paths (`22c4a25`)
- Migrate OpenAI catalog retirement filter (`e75ef82`)
- Migrate ps process row parsing (`f6925a8`)
- Migrate DeepSeek stream error classification (`a6efc28`)
- Migrate provider models response policy (`f3ab715`)
- Migrate app command startup policy (`6340764`)
- Migrate shared Codex filename policy (`e5c960d`)
- Migrate imported secret file schema policy (`bb58986`)
- Migrate chat bridge response projection to Mojo (`0cd8e97`)
- Move model prefix scan to Mojo (`97deded`)
# Prodex 0.438.0

## New Features

### Codex 0.163.0 compatibility

The release qualifies the official `rust-v0.163.0` source and released
Linux executable, including a real isolated app-server initialize and
thread-list handshake without starting model turns.

- Native `response.incomplete` reasons, response IDs, unknown fields and
  usage remain intact. The proxy does not invent success or rotate a
  profile merely because the native model result is incomplete.
- Foreground and background Responses-v2 compaction keep native usage,
  retention, image budget, cancellation and error handling.
- The native default instant-interrupt and eligible Daybreak behavior is
  preserved rather than overridden by the wrapper.
- Reasoned WebSocket continuation-property mismatches, model-resolved
  incremental tool messages and explicit remote MCP Windows environment
  overrides are covered by the updated source guard.
- Active native connection retries remain owned by Codex. Local transport
  repair does not manufacture another turn or override committed output
  and hard continuation affinity.

The compatibility baseline checks 77 critical files and 90 mandatory
semantic contracts against the exact tagged source. Source archive and
file digests are recorded in the migration audit.

## Bug Fixes

### Local proxy recovery under descriptor pressure

The owned local HTTP listener now survives temporary failures in both
accepting a connection and duplicating an accepted socket. Previously the
HTTP server dependency could terminate its accept thread, leaving the
native client retrying an unreachable loopback address instead of reaching
profile selection. An isolated descriptor-pressure test reproduced the
socket-clone panic before the patch.

The repair preserves the listening address and queued connection, retries
with bounded 10–250 ms backoff, limits diagnostic volume and observes the
server shutdown flag. It does not replay parsed HTTP requests, submitted
inference, emitted output or completed tool calls. Two real TCP tests
independently force accept and accepted-stream descriptor exhaustion,
release the pressure and require the queued request plus a new connection
to succeed on the same port.

The retained incident logs prove repeated failures to the local proxy,
not the exact OS event that killed the original listener. Persistent
resource exhaustion, a killed process, an actual upstream outage or an
ineligible profile pool are not claimed to be error-free.

### Windows exit and cancellation recovery

The compiled Mojo launch policy now accepts the signed 32-bit process exit
codes supplied by Windows, including the negative `CONTROL_C_EXIT` status.
Ordinary exits and recoverable failures no longer panic while checking for
cancellation. Ctrl+C still prevents automatic relaunch; absent Unix status
fields, malformed ABI inputs and interrupted sessions retain their original
safety behavior. Portable signed-status tests and a native Windows caller
regression exercise the same production policy.

## Changelog

### Qualification and maintenance

The tiny_http 0.12.0 transport patch retains the official runtime sources,
licenses and archive provenance. Exact-file hash verification and tamper
regressions run in preflight. Original audited vendor lint annotations do
not increase first-party suppression budgets.

Inherited oversized adapters and tests were extracted into cohesive
modules without changing their public APIs or dropping assertions.
Source-size limits remain unchanged. Retry, quota transitions, compaction,
hard affinity, usage-limit recovery and session recovery remain release
gates, alongside compiled-Mojo tests and workspace Clippy.

The optional Codebase Memory MCP latest-stable reference is refreshed to
0.12.0 from its official release. Qualification does not update the user's
installed shared daemon or rebuild existing indexes.

### Upgrade

Use the normal `prodex update` flow, then verify `prodex --version` reports
`prodex 0.438.0`. Existing 0.437.x processes are not hot-patched. Resume the
saved session with `prodex s <session-id>` on the new version; do not use a
closed temporary `codex --remote` socket or delete persisted session data.

Full Changelog: [0.437.14...0.438.0](https://github.com/christiandoxa/prodex/compare/0.437.14...0.438.0)

## 0.437.14 - 2026-10-15

### Runtime

- Recover authenticated profiles across quota transitions (`c49981d`)

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
