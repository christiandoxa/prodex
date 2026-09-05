---
name: prodex-session-prompt-write
description: Safely write a user prompt to and observe an already-running plain prodex s session through its default expose MCP bridge.
---

# Existing Prodex session bridge

Use `prodex s expose` from the same canonical workspace as the one plain
`prodex s` session. The default MCP tools are:

- `prodex_session_prompt_write({"message":"..."})` for Prompt Write;
- `prodex_session_output_read({"cursor":"...","wait_ms":5000})` to read
  later sanitized visible user, assistant, tool, MCP/agent, and session/turn-status events.

For development requests, resolve the existing session first. Use
`prodex_session_prompt_write` exactly once when one compatible plain `prodex s`
exists, then read output with the returned `prodex_pid`, `thread_id`, and
`next_cursor`. The prompt write is observable in the already-open parent
TUI. A fresh idle session is valid before its first manual prompt;
the bridge verifies its live app-server thread and delivers the requested message
through Codex. Start one `prodex_super_start` fallback only after an authoritative
`no_session` result. Never start the existing-session and fallback paths in
parallel, and never treat addressability, ambiguity, stale identity, queue,
source, or verification errors as `no_session`.

The two tools share one target identity. Require the same live OS user,
canonical cwd, plain-session command, Prodex process birth identity, actual
Codex writer ancestry/birth identity, Codex home authority, source identity,
and thread UUID. Ambiguity, restart, source rotation, or a changed writer is a
fail-closed error; never guess from the newest rollout.

Modern Codex thread authority is the writer's open
`thread-writer-locks/<UUID>.lock`. Legacy authority is exactly one open
`rollout-...-<UUID>.jsonl`. If both are present, their UUIDs must agree. A
normal persisted session is required after writing. A fresh Codex 0.153.2
session may be held in the writer's live app-server until its first written
message; the bridge verifies that exact live thread before writing and then
revalidates persistence.

Output reads are bounded and cursor-based. Return only existing sanitized
user-visible assistant/tool/status transcript events. Never expose hidden
reasoning, instructions, credentials, queue payloads, or raw rollout JSON.
Never read a target PTY or `/dev/pts`, synthesize keystrokes, write SQLite
queue rows, or start another solver/writer.
