# Native root-session recovery ownership (0.437.9)

## Incident and primary-source contract

Read-only BRP inspection on 2026-10-13 found an ordinary non-goal native Codex
0.162.1 thread with repeated `task_complete.error.codex_error_info=server_overloaded`
records and no terminal `status` field. Its Prodex 0.437.8 recovery marker named
a different UUID. That UUID had neither a persisted rollout nor a state-index row,
and the owned app-server rejected `thread/read` with `thread not loaded`. The
actual root thread was persistent and accepted direct input. No upstream model
request, session mutation, configuration change or process restart was performed
by this inspection.

The exact native source is Codex `rust-v0.162.1`, commit
`092d3acd6bec3e3a14bdc7e7a2810ab628ab759d`:

- `codex-rs/core/src/session/session.rs` distinguishes concrete `thread_id()`
  from `session_id()`, the identity shared by the root and its descendants.
- `codex-rs/core/src/hook_runtime.rs` supplies `sess.session_id()` to the
  SessionStart hook input.
- `codex-rs/hooks/src/legacy_notify.rs` supplies the completing agent's concrete
  `event.thread_id` as `thread-id`. Descendants can inherit the same notify command.

Prodex previously preferred `thread-id` over `session_id`, unconditionally rewrote
the shared recovery marker, and recomputed its cursor at end-of-file on every
callback. A descendant completion could therefore redirect the root monitor to
an unrelated or nonpersisted thread. Even a repeated root completion or compact
hook could seek past a terminal failure before the monitor observed it.

## Corrected behavior

The existing versioned Mojo launch-policy owner now decides marker replacement.
A canonical SessionStart may initialize or explicitly switch the root. Repeated
notifications for the same root preserve its cursor, and legacy concrete-thread
notifications cannot replace an established different root. Legacy-only startup
is still supported when no valid marker exists.

Rust validates the hook JSON and UUID, serializes competing hook writers using
the existing host file-lock facility, and atomically publishes the logical rollout
cursor before the root ID. It never treats compressed byte length as a decoded
JSONL offset. The owned monitor removes its sidecar lock on drop.

Recovery remains on the existing owned app-server and persisted thread. Active
turns, paused/completed goals, cancellation, explicit no-auto-rotate, ambiguous
RPC acceptance, cross-thread ownership and already-committed side effects retain
their existing protections. No original task is requeued, no terminal keystroke
is synthesized, no model is silently replaced, and native context-window and
compaction settings are unchanged.

## Behavioral evidence

Before the patch, three new production-boundary tests failed: foreign legacy
notify redirected the root, a repeated callback advanced the unread cursor, and
a concrete thread ID incorrectly outranked canonical session identity. Twelve
pre-existing native terminal regressions still passed on that baseline.

With the patch, the focused recovery suite passed 44 tests, native socket
recovery passed 9, goal/hook/affinity regressions passed 8, and the Mojo recovery
policy suite passed 7. New tests also cover legacy bootstrap, an explicit new
root, malformed canonical input and concurrent descendant notifications.

Two deliberate mutations of the actual Mojo marker policy independently restore
the foreign-owner and cursor-loss bugs. Each failed the sensitivity fixture;
after byte-exact source restoration, both passed. No Rust semantic fallback is
introduced. Fixtures contain no private prompts, credentials or copied user
payloads.

Available account quota and upstream model capacity are separate conditions.
This repair prevents local recovery misrouting; it does not guarantee success
when the chosen model is overloaded across every eligible account.
