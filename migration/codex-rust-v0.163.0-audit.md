# Codex rust-v0.163.0 compatibility qualification

Prodex 0.438.0 qualifies the exact official `rust-v0.163.0` source and
release binary. The annotated tag resolves to
`83086188ae0614bd882818ff1aa23f3194962012`. Archive digests, source-file
digests and comparison counts are recorded in
`codex-rust-v0.163.0-source-evidence.json` beside this audit.

## Native contracts retained

- Responses `response.incomplete` retains its reason, response ID and usage.
  A non-interrupted incomplete result remains a native terminal error;
  `content_filter` and `interrupted` retain their distinct upstream behavior.
  Prodex does not substitute success, quota exhaustion or profile rotation.
- Foreground and background Responses-v2 compaction preserve incomplete
  usage and leave compaction orchestration, retention and image budgets
  with Codex. Typed `RetainedImageBudget` replaces the old local feature
  lookup in the audited compaction function.
- Instant interrupt is now enabled by default. Daybreak is stable and
  enabled for native eligible contexts. No wrapper flag overrides either
  native default, permissions, cancellation or provider eligibility.
- WebSocket continuation property checks now return mismatch reasons.
  Unknown request fields, original continuation ownership and native
  reconnect/fallback handling remain intact.
- Remote MCP launch applies explicit environment overrides through its
  Windows-aware merge. Its bootstrap allowlist now also carries
  `SYSTEMDRIVE` and `LOCALAPPDATA`; unrelated secrets remain excluded.
- Incremental tool notices use model-resolved messages instead of fixed
  local constants. Catalog hashes and tool namespace ownership are native.
- App-server initialization and `thread/list` succeed against the actual
  released 0.163.0 executable in isolated configuration and SQLite homes.
  No model turn, account login, installed binary replacement or live
  inference was performed for this qualification.

## Retry ownership and the reported incident

Native sampling connection failures can intentionally stay in an active
turn while waiting for the network, with delays capped at 60 seconds.
The retained incident logs show connection failures to the local Prodex
listener, not proof of exhausted upstream profiles. Rotation cannot act
until a request reaches the local proxy. Quiet native retries must not be
interpreted as permission to inject another turn or repeat completed tools.

Prodex's transport repair keeps the local listening socket alive across
accept/descriptor-duplication errors and preserves queued connections.
It does not globally disable native connection retries, cross a hard
profile affinity boundary, replay a committed response, or falsify quota.
The source of descriptor pressure in the original exited process is not
recoverable from the retained logs; the failure path was independently
reproduced under a subprocess-only descriptor limit before being fixed.

## Verification boundary

The source guard covers 77 critical files and 90 mandatory semantic
contracts, including four new contracts for this release. Required marker
changes are backed by the tagged source, not removed to hide drift.
The guard's self-tests and capture/replay fixtures remain required.
The production SSE regression preserves incomplete usage and unknown
fields without manufacturing retry/rotation. Existing affinity,
compaction, quota-transition and post-commit tests remain release gates.
